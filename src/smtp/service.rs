use std::io::{Error as IoError};
use std::mem;

use futures::sync::{mpsc, oneshot};
use futures::{Future, Stream, Poll, Async};

use tokio_proto::util::client_proxy::ClientProxy;
use tokio_service::{Service as TokioService};
use tokio_proto::streaming::{Body, Message};

use tokio_smtp::request::{
    Request as SmtpRequest,
};
use tokio_smtp::response::{Response as SmtpResponse};

use ::CompositionBase;
use super::smtp_wrapper::{WrappedService, MailSendRequest, MailSendError, MailResponse};
use super::handle::MailServiceHandle;

//Next steps:
// make the service accept encodable mails nothing else
// do the bla -> encodable in the setup part check for canceleation before sending

pub type TokioSmtpService = ClientProxy<
    Message<SmtpRequest, Body<Vec<u8>, IoError>>,
    SmtpResponse, IoError>;

trait SmtpSetup: Send {

    /// The future returned which returns a Smtp connection,
    ///
    /// as this smtp mail bindings are writting for `tokio_smtp`
    /// the Item is fixed to `ClientProxy`, (This might change
    /// in future versions)
    type ConnectFuture: 'static + Future<
        Item=TokioSmtpService,
        Error=Self::NotConnectingError>;

    /// The error returned if it is not possible to connect,
    /// this might represent a direct connection failure or
    /// one involfing multiple retries or similar aspects.
    type NotConnectingError;

    /// The impl of the CompositionBAse
    type CompositionBase: CompositionBase;

    // this future can contain all kind of retry connection handling etc.
    /// This method is called to connect with an SMTP server.
    ///
    /// It is called whenever connecting to a SMTP server is necessary,
    /// this includes the initial connection as well as reconnecting after
    /// the connection might no longer be usable.
    ///
    /// As it returns a future with it's own error it can be used to
    /// handle automatically retrying failed connections and limiting
    /// the amount of retries or having a timeout before retrying to
    /// connect.
    ///
    //TODO
    /// Currently it is not implemented to retry sending failed mails, even
    /// if it reconnects after e.g. an IO error
    fn connect(&mut self) -> Self::ConnectFuture;

    /// return the buffer size for the mpsc channel between the service and it's handles
    fn driver_input_buffer_size(&self) -> usize { 16 }

    /// Return a `CompositionBase` to use, this will only be called once for
    /// the lifetime of a service.
    fn composition_base(&self) -> Self::CompositionBase;
}



struct MailService<SUP, RQ>
    where SUP: SmtpSetup, RQ: MailSendRequest,
{
    setup: SUP,
    rx: mpsc::Receiver<(RQ, oneshot::Sender<Result<MailResponse, MailSendError>>)>,
    service: ServiceState<SUP::CompositionBase, SUP::ConnectFuture, RQ>,
    pending: Option<(
        Box<Future<Item=MailResponse, Error=MailSendError>>,
        oneshot::Sender<Result<MailResponse, MailSendError>>
    )>
}

enum ServiceState<CB, F, RQ> {
    Initial(CB),
    Connecting(CB, F),
    Connected(WrappedService<TokioSmtpService, CB, RQ>),
    Dead
}

impl<CB, F, RQ> ServiceState<CB, F, RQ> {

    fn reset(&mut self) {
        use self::ServiceState::*;
        let state = mem::replace(self, ServiceState::Dead);
        *self = match state {
            Initial(cb) => Initial(cb),
            Connecting(cb, _f) => Initial(cb),
            Connected(ws) => {
                let (_, cb) = ws.destruct();
                Initial(cb)
            },
            Dead => Dead
        }
    }
}

impl<SUP, RQ> MailService<SUP, RQ>
    where SUP: SmtpSetup,
          RQ: MailSendRequest
{

    pub fn new(setup: SUP) -> (Self, MailServiceHandle<RQ>) {
        let cb = setup.composition_base();
        let (tx, rx) = mpsc::channel(setup.driver_input_buffer_size());

        let driver = MailService {
            setup, rx,
            service: ServiceState::Initial(cb),
            pending: None,
        };

        let handle = MailServiceHandle::new(tx);
        (driver, handle)
    }

    fn poll_connect(&mut self) -> Poll<(), SUP::NotConnectingError> {
        use self::ServiceState::*;
        let mut state = mem::replace(&mut self.service, ServiceState::Dead);
        let mut result = None;
        while result.is_none() {
            state = match state {
                Initial(cb) => {
                    Connecting(cb, self.setup.connect())
                },
                Connecting(cb, mut fut) => {
                    match fut.poll() {
                        Ok(Async::Ready(service)) => {
                            result = Some(Ok(Async::Ready(())));
                            Connected(WrappedService::new(service, cb))
                        },
                        Ok(Async::NotReady) => {
                            result = Some(Ok(Async::NotReady));
                            Connecting(cb, fut)
                        }
                        Err(err) => {
                            result = Some(Err(err));
                            Dead
                        }
                    }
                },
                Connected(service) => {
                    result = Some(Ok(Async::Ready(())));
                    Connected(service)
                },
                Dead => {
                    panic!("polled Service after completion through Err or Panic+catch_unwing")
                }
            }
        }
        self.service = state;
        result.unwrap()
    }

    fn service_mut(&mut self)
        -> &mut WrappedService<TokioSmtpService, SUP::CompositionBase, RQ>
    {
        use self::ServiceState::*;
        match &mut self.service {
            &mut Connected(ref mut service) => service,
            _ => panic!("[BUG] service_mut can only be called if we are connected")
        }
    }

    fn poll_pending_complete(&mut self) -> bool {
        let res =
            if let Some(&mut (ref mut pending, _)) = self.pending.as_mut() {
                match pending.poll() {
                    Ok(Async::Ready(res)) => Ok(res),
                    Ok(Async::NotReady) => return false,
                    Err(err) => {
                        // if an error happend on smtp RSET we can just reconnect
                        let reset_conn =
                            match &err {
                                &MailSendError::OnReset(_) => true,
                                &MailSendError::Io(_) => true,
                                // we should not reach here but intentionally just
                                // ignore the fact that someone pretents to be us
                                // and generates this errors
                                &MailSendError::DriverDropped |
                                &MailSendError::CanceledByDriver => false,
                                _ => false
                            };

                        if reset_conn {
                            self.service.reset();
                        }
                        Err(err)
                    }
                }
            } else {
                return true;
            };

        //UNWRAP_SAFE: we can only be here if aboves `if let Some` succeeded
        let (_pending, req_rx) = self.pending.take().unwrap();
        // we do not care about
        // cancellation at this point
        let _ = req_rx.send(res);
        true
    }
}

impl<SUP, RQ> Future for MailService<SUP, RQ>
    where RQ: MailSendRequest,
          SUP: SmtpSetup
{
    type Item = ();
    type Error = SUP::NotConnectingError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            // 1. complete the "current"/pending request (if there is any)
            if !self.poll_pending_complete() {
                return Ok(Async::NotReady)
            }

            // 2. make sure we are connected, the current request might have "broken" the connection
            try_ready!(self.poll_connect());

            // 3. try to get a new request
            let item = match self.rx.poll() {
                Ok(Async::Ready(item)) => item,
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(_) => unreachable!("mpsc::Receiver.poll does not error")
            };

            // stop driver if all channels to it got closed
            // with the current mpsc impl. (tokio 0.1.14) the receiver won't
            // know about it but future impl. might be more clever
            let (request, req_rx) =
                if let Some(ele) = item { ele }
                else { return Ok(Async::Ready(())) };

            // 4. set call the service with the new request and set it to pending
            self.pending = Some((self.service_mut().call(request), req_rx));
        }
    }
}
use std::io::{Error as IoError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::mem;

use futures::sync::{mpsc, oneshot};
use futures::{future, Future, Stream, Poll, Async};
use futures::stream::BufferUnordered;

use tokio_proto::util::client_proxy::ClientProxy;
use tokio_proto::streaming::{Body, Message};

use tokio_smtp::request::{Request as SmtpRequest,};
use tokio_smtp::response::{Response as SmtpResponse};

use mail::prelude::{Encoder, Encodable, MailType};
use mail::utils::SendBoxFuture;
use mail::context::BuilderContext;

use super::smtp_wrapper::send_mail;
use super::common::{MailResponse, MailRequest, EnvelopData};
use super::handle::MailServiceHandle;
use super::error::MailSendError;


//Next steps:
// make the service accept encodable mails nothing else
// do the bla -> encodable in the setup part check for canceleation before sending

pub type TokioSmtpService = ClientProxy<
    Message<SmtpRequest, Body<Vec<u8>, IoError>>,
    SmtpResponse, IoError>;

pub trait SmtpSetup: Send {

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

    type BuilderContext: BuilderContext;

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

    fn context(&self) -> Self::BuilderContext;

    /// return how many mail should be encoded at the same time
    ///
    /// encoding a `Mail` includes transforming it into an `EncodableMail` which means
    /// loading all resources associated with the `Mail`
    fn mail_encoding_buffer_size(&self) -> usize { 16 }

    /// return the buffer size for the mpsc channel between the service and it's handles
    ///
    /// By default each handle has one and the loading buffer is directly connected to the
    /// receiver, but the difference between the buffers is that sender can write into the
    /// mpsc channels buffer _in their thread_ while moving the data buffered in the mpsc
    /// channel to the `BufferUnordered` buffer is done _while polling the service driver_.
    fn mail_enqueuing_buffer_size(&self) -> usize { 16 }
}



type ServiceInputMessage = (MailRequest, oneshot::Sender<MailSendResult>);
pub type MailSendResult = Result<MailResponse, MailSendError>;

pub struct MailService<SUP>
    where SUP: SmtpSetup
{
    setup: SUP,
    rx: BufferUnordered<StreamEncodeMail<SUP::BuilderContext>>,
    service: ServiceState<SUP::ConnectFuture>,
    pending: Option<(
        Box<Future<Item=MailResponse, Error=MailSendError>>,
        oneshot::Sender<MailSendResult>
    )>,
    stop_handle: StopServiceHandle
}


impl<SUP> MailService<SUP>
    where SUP: SmtpSetup
{

    pub fn new(setup: SUP) -> (Self, MailServiceHandle) {
        let ctx = setup.context();

        let (tx, rx) = mpsc::channel(setup.mail_enqueuing_buffer_size());

        let rx = StreamEncodeMail::new(rx, ctx)
            .buffer_unordered(setup.mail_encoding_buffer_size());

        let driver = MailService {
            setup, rx,
            service: ServiceState::Initial,
            pending: None,
            stop_handle: StopServiceHandle::new()
        };

        let handle = MailServiceHandle::new(tx);
        (driver, handle)
    }

    pub fn stop_handle(&self) -> StopServiceHandle {
        self.stop_handle.clone()
    }

    fn poll_connect(&mut self) -> Poll<(), SUP::NotConnectingError> {
        use self::ServiceState::*;
        let mut state = mem::replace(&mut self.service, ServiceState::Dead);
        let mut result = None;
        while result.is_none() {
            state = match state {
                Initial => Connecting(self.setup.connect()),
                Connecting(mut fut) => {
                    match fut.poll() {
                        Ok(Async::Ready(service)) => {
                            result = Some(Ok(Async::Ready(())));
                            Connected(service)
                        },
                        Ok(Async::NotReady) => {
                            result = Some(Ok(Async::NotReady));
                            Connecting(fut)
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
                    panic!("polled Service after completion through Err or Panic+catch_unwind")
                }
            }
        }
        self.service = state;
        //UNWRAP_SAFE: loop only exits when result is some
        result.unwrap()
    }

    fn service_mut(&mut self) -> &mut TokioSmtpService
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
                            //IMPROVE: we might want consider sending Quit, through it's not needed
                            //  - one way to land here is a IoError in which we can't even do it
                            //  - the other is if
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
        // we do not care about cancellation at this point
        // so just drop the result of send
        let _ = req_rx.send(res);
        true
    }
}

impl<SUP> Future for MailService<SUP>
    where SUP: SmtpSetup
{
    type Item = ();
    type Error = SUP::NotConnectingError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if self.stop_handle.should_stop() {
            // close the underlying streams ability to receive new messages,
            // but the buffer still contains messages so continue with the rest
            self.rx.get_mut().get_mut().close()
        }
        loop {
            // 1. complete the "current"/pending request (if there is any)
            if !self.poll_pending_complete() {
                return Ok(Async::NotReady)
            }

            // 2. make sure we are connected, the current request might have "broken" the connection
            try_ready!(self.poll_connect());

            // 3. try to get a new request
            let item = match self.rx.poll() {
                Ok(Async::Ready(Some(item))) => item,
                Ok(Async::Ready(None)) => return Ok(Async::Ready(())),
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(_) => unreachable!("mpsc::Receiver.poll does not error")
            };

            // stop driver if the stream ends, which could happen when
            // - it was closed and the buffer is empty
            let (data, envelop, req_rx) =
                if let Some(item) = item { item
                // we might have dropped some parts pre maturealy due to e.g.
                // encoding errors
                } else { continue; };

            // 4. set call the service with the new request and set it to pending
            let pending_future = send_mail(self.service_mut(), data, envelop);
            self.pending = Some((pending_future, req_rx));
        }
    }
}

//NOTE: we would not need this if we could have abstract types
// i.e. existential impl Type on module/struct level
struct StreamEncodeMail<CTX>
    where CTX: BuilderContext
{
    stream: mpsc::Receiver<ServiceInputMessage>,
    ctx: CTX
}

impl<CTX> StreamEncodeMail<CTX>
    where CTX: BuilderContext
{
    fn new(stream: mpsc::Receiver<ServiceInputMessage>, ctx: CTX) -> Self {
        StreamEncodeMail { stream, ctx}
    }

//    fn get_ref(&self) -> &mpsc::Receiver<ServiceInputMessage> {
//        &self.stream
//    }

    fn get_mut(&mut self) -> &mut mpsc::Receiver<ServiceInputMessage> {
        &mut self.stream
    }
}

impl<CTX> Stream for StreamEncodeMail<CTX>
    where CTX: BuilderContext
{
    //FIXME[tokio v0.2]: () => Never
    type Item = SendBoxFuture<Option<(Vec<u8>, EnvelopData, oneshot::Sender<MailSendResult>)>, ()>;
    //FIXME[tokio v0.2]: () => Never
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let (mail_request, tx) = match self.stream.poll() {
            Ok(Async::Ready(Some(mail_request))) => mail_request,
            Ok(Async::Ready(None)) => return Ok(Async::Ready(None)),
            Ok(Async::NotReady) => return Ok(Async::NotReady),
            Err(_) => panic!("[BUG/futures] mpsc::Receiver can not error")
        };

        let ctx = self.ctx.clone();

        //use lazy to make sure it's run in the thread pool
        let operation =
            future::lazy(move || mail_request.into_mail_with_envelop())
            .then(move |result| match result {
                Ok((mail, envelop)) => Ok((mail, envelop, tx)),
                Err(err) => Err((MailSendError::CreatingEnvelop(err), tx))
            })
            .and_then(move |(mail, envelop, tx)| {
                mail.into_encodeable_mail(&ctx)
                    .then(move |result| match result {
                        Ok(enc_mail) => Ok((enc_mail, envelop, tx)),
                        Err(err) => Err((MailSendError::Encoding(err), tx))
                    })
            })
            .and_then(move |(encodable_mail, envelop, tx)| {
                //TODO we need to feed in the MailType (and get it from tokio smtp)
                let mut encoder = Encoder::new( MailType::Ascii );
                match encodable_mail.encode(&mut encoder) {
                    Ok(()) => {},
                    Err(err) => return Err((MailSendError::Encoding(err), tx))
                }

                let bytes = match encoder.to_vec() {
                    Ok(bytes) => bytes,
                    Err(err) => return Err((MailSendError::Encoding(err), tx))
                };

                //TODO we also need to return SmtpEnvelop<Vec<u8>>
                Ok(Some((bytes, envelop, tx)))
            })
            .or_else(move |(err, tx)| {
                // if the receiver was dropped just drop the error, too
                let _ = tx.send(Err(err));
                // we will filter that out
                Ok(None)
            });

        // offload actual encoding to thread pool
        Ok(Async::Ready(Some(self.ctx.offload(operation))))
    }
}

enum ServiceState<F> {
    Initial,
    Connecting(F),
    Connected(TokioSmtpService),
    Dead
}

impl<F> ServiceState<F> {

    fn reset(&mut self) {
        use self::ServiceState::*;
        let state = mem::replace(self, ServiceState::Dead);
        *self = match state {
            Dead => Dead,
            _ => Initial
        }
    }
}



#[derive(Debug, Clone)]
pub struct StopServiceHandle(Arc<AtomicBool>);

impl StopServiceHandle {
    pub fn new() -> Self {
        StopServiceHandle(Arc::new(AtomicBool::new(false)))
    }
    pub fn should_stop(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn stop(&self) {
        self.0.store(true, Ordering::Release)
    }
}



#[cfg(test)]
mod test {
    use std::io;
    use futures::{Future, IntoFuture};
    use mail::prelude::*;
    use chrono::{Utc, TimeZone};
    use super::super::test::*;
    use super::*;


    fn _test<F, R, S, D>(setup: S, fail_connect: bool, func: F, other_driver: D)
        where S: SmtpSetup,
              F: FnOnce(MailServiceHandle) -> R,
              R: IntoFuture<Item=(), Error=TestError>,
              D: Future<Item=(), Error=TestError>
    {
        let (driver, handle) = MailService::new(setup);
        let stop_handle = driver.stop_handle();

        let test_fut = func(handle)
            .into_future()
            .then(|res| {
                stop_handle.stop();
                res
            });

        let driver = driver.then(|res| match res {
            Ok(_) if fail_connect =>
                Err(TestError("[test] did not fail to connect".to_owned())),
            Err(_) if !fail_connect =>
                Err(TestError("[test] did unexpected fail to connect".to_owned())),
            _ => Ok(())
        });

        // we want all futures to complete independent of errors
        // so there errors get "lifted" into their item
        let driver = driver.then(|res| Ok(res));
        let test_fut = test_fut.then(|res| Ok(res));
        let other_driver = other_driver.then(|res| Ok(res));

        let res: Result<_, ()> = driver.join3(test_fut, other_driver).wait();
        let (rd, rt, rod) = res.unwrap();
        match rd { Ok(_) => {}, Err(TestError(msg)) => panic!(msg) }
        match rt { Ok(_) => {}, Err(TestError(msg)) => panic!(msg) }
        match rod { Ok(_) => {}, Err(TestError(msg)) => panic!(msg) }
    }

    fn example_io_error() -> IoError {
        IoError::new(io::ErrorKind::Other, "it broke")
    }

    fn example_mail() -> (MailRequest, &'static str) {
        let headers = headers! {
            From: ["djinns@are.magic"],
            To: ["lord.of@the.bottle"],
            Subject: "empty bottle, no djinn",
            Date: Utc.ymd(2023, 1, 1).and_hms(1, 1, 1)
        }.unwrap();

        let mail = Builder
        ::singlepart(text_resource("<--body-->"))
            .headers(headers).unwrap()
            .build().unwrap();

        let req = MailRequest::new(mail);

        let expected_body = concat!(
            "MIME-Version: 1.0\r\n",
            "From: <djinns@are.magic>\r\n",
            "To: <lord.of@the.bottle>\r\n",
            "Subject: empty bottle, no djinn\r\n",
            "Date: Sun,  1 Jan 2023 01:01:01 +0000\r\n",
            "Content-Type: text/plain\r\n",
            "Content-Transfer-Encoding: 7bit\r\n",
            "\r\n",
            "<--body-->\r\n"
        );

        (req, expected_body)

    }
    #[test]
    fn send_simple_mail() {
        use self::RequestMock::*;
        let (req, expected_body) = example_mail();
        let (setup, fake_server) = TestSetup::new(1,
            vec![
                Normal(SmtpRequest::Mail {
                    from: "djinns@are.magic".parse().unwrap(), params: Vec::new() }),
                Normal(SmtpRequest::Rcpt {
                    to: "lord.of@the.bottle".parse().unwrap(), params: Vec::new() }),
                Body(SmtpRequest::Data, expected_body.to_owned().into_bytes())
            ],
            vec![
                Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
                Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
                Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
            ]
        );

        _test(setup, false, |handle| {
            handle.send_mail(req)
                .and_then(|(_handle, resp_fut)| resp_fut)
                .and_then(|_resp: MailResponse| {
                    //MailResponse is currently zero sized, so nothing to do here
                    Ok(())
                })
                .map_err(|mse| TestError(format!("unexpected error: {:?}", mse)))
        }, fake_server)
    }

    #[test]
    fn reset_connection_on_io_error() {
        use self::RequestMock::*;
        let (req, expected_body) = example_mail();
        let (setup, fake_server) = TestSetup::new(2,
            vec![
                Normal(SmtpRequest::Mail {
                    from: "djinns@are.magic".parse().unwrap(), params: Vec::new() }),
                // currently we only check for errs after sending all non Data parts
                Normal(SmtpRequest::Rcpt {
                    to: "lord.of@the.bottle".parse().unwrap(), params: Vec::new() }),
                Normal(SmtpRequest::Mail {
                    from: "djinns@are.magic".parse().unwrap(), params: Vec::new() }),
                Normal(SmtpRequest::Rcpt {
                    to: "lord.of@the.bottle".parse().unwrap(), params: Vec::new() }),
                Body(SmtpRequest::Data, expected_body.to_owned().into_bytes())
            ],
            vec![
                Err(example_io_error()),
                Err(example_io_error()),
                Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
                Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
                Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
            ]
        );

        _test(setup, false, |handle| {
            handle.send_mail(req)
                .map_err(|err| TestError(format!("unexpected enque error {:?}", err)))
                .and_then(|(handle, resp_fut)| resp_fut.then(|res| match res {
                    Ok(MailResponse) => Err(TestError("[test] unexpected no error".to_owned())),
                    Err(err) => {
                        if let MailSendError::Io(_) = err {
                            Ok(handle)
                        } else {
                            Err(TestError(format!("unexpected error kind {:?}", err)))
                        }
                    }
                }))
                .and_then(|handle| {
                    let (req, _) = example_mail();
                    handle.send_mail(req)
                        .map_err(|err| TestError(format!("unexpected enque error {:?}", err)))
                })
                .and_then(|(_handle, res_fut)| {
                    res_fut.map_err(|err| TestError(format!("unexpected error {:?}", err)))
                })
                .map(|_| ())
        }, fake_server)
    }

    #[test]
    fn failed_reset_connection() {
        use self::RequestMock::*;
        let (req, _) = example_mail();
        let (setup, fake_server) = TestSetup::new(1,
            vec![
                Normal(SmtpRequest::Mail {
                    from: "djinns@are.magic".parse().unwrap(), params: Vec::new() }),
                // currently we only check for errs after sending all non Data parts
                Normal(SmtpRequest::Rcpt {
                    to: "lord.of@the.bottle".parse().unwrap(), params: Vec::new() }),
            ],
            vec![
                Err(example_io_error()),
                Err(example_io_error()),
            ]
        );

        _test(setup, true, |handle| {
            handle.send_mail(req)
                .and_then(|(_handle, res_fut)| res_fut)
                .then(|res| match res {
                    Ok(_) => Err(TestError("unexpected no error".to_owned())),
                    Err(err) => {
                        if let MailSendError::Io(_) = err {
                            Ok(())
                        } else {
                            Err(TestError(format!("unexpected error kind: {:?}", err)))
                        }
                    }
                })
        }, fake_server)

    }
}
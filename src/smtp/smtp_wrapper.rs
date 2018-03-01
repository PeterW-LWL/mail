use std::marker::PhantomData;
use std::io::{Error as IoError};

use futures::future::{self, Either, Future};

use tokio_service::{Service as TokioService};

use tokio_smtp::request::{
    Request as SmtpRequest,
    Mailbox as SmtpMailbox
};
use tokio_smtp::response::{Response as SmtpResponse};
use tokio_proto::streaming::{Body, Message};

use mail::Mail;
use mail::headers::components::Mailbox;
use ::error::Error;
use ::{CompositionBase, EnvelopData};


#[derive(Clone, Debug)]
pub struct MailResponse;

///
/// # Example (Double Dispatch to allow multiple MailSendData Data types)
///
/// ```
//TODO imports etc.
// # type UserData = ();
// # type OtherData = ();
// enum Request {
//     UserData(UserData),
//     OtherData(OtherData)
// }
//
// impl MailSendRequest for Request {
//     fn call_compose_mail<CB>(&self, cb: &CB) -> Result<(Mail, EnvelopData), Error>
//         where CB: CompositionBase
//     {
//         use self::Request::*;
//         match *self {
//             UserData(ref data) => cb.compose_mail(data),
//             OtherData(ref data) => cb.compose_mail(data)
//         }
//     }
// }
/// ```
///
///
pub trait MailSendRequest {

    /// Call the composition bases `compose_mail` method with the contained requests SendMailData
    ///
    /// If all teplates/send mail use the same type of Data this would just be implemented one the
    /// data with `cb.compose_mail(self)` but if there are multiple different type for e.g.
    /// different templates you can wrapp them in a enum implement `MailSendRequest` on the enum
    /// and then match on the enum and calling `compose_mail` depending on the actual used type.
    ///
    fn call_compose_mail<CB>(&self, cb: &CB) -> Result<(Mail, EnvelopData), Error>
        where CB: CompositionBase;
}

pub struct WrappedService<I, CB, R> {
    inner: I,
    composition_base: CB,
    _limiter: PhantomData<R>
}

impl<I, CB, R> WrappedService<I, CB, R> {
    pub fn destruct(self) -> (I, CB) {
        (self.inner, self.composition_base)
    }
}

impl<I, CB, R> WrappedService<I, CB, R>
    where I: TokioService<
        Request=Message<SmtpRequest, Body<Vec<u8>, IoError>>,
        Response=SmtpResponse
    >,
          CB: CompositionBase,
          R: MailSendRequest
{

    pub fn new(service: I, composition_base: CB) -> WrappedService<I, CB, R> {
        WrappedService {
            inner: service,
            composition_base,
            _limiter: PhantomData
        }
    }
}

impl<I: 'static, CB, R> TokioService for WrappedService<I, CB, R>
    where I: Clone + TokioService<
        Request=Message<SmtpRequest, Body<Vec<u8>, IoError>>,
        Response=SmtpResponse,
        Error=IoError
    >,
          CB: CompositionBase,
          R: MailSendRequest
{
    type Request = R;
    type Response = MailResponse;
    type Error = MailSendError;
    type Future = Box<Future<Item=MailResponse, Error=Self::Error>>;

    /// Process the request and return the response asynchronously.
    fn call(&self, req: Self::Request) -> Self::Future {
        let (mail, envelop_data) = r2f_try!(
            req
            .call_compose_mail(&self.composition_base)
            .map_err(|e| MailSendError::Composition(e))
        );

        let service = &self.inner;

        Box::new(mail
            .into_encodeable_mail(self.composition_base.context())
            .map_err(|err| MailSendError::Encoding(err))
            .and_then(move |encodable_mail| {
                use mail::prelude::{Encoder, MailType, Encodable};
                //TODO we need to get information about SMTPUTF8 support and 8BIT support, tokio-smtp
                // currently does not support this
                let mut encoder = Encoder::new( MailType::Ascii );
                encodable_mail.encode(&mut encoder)
                    .map_err(|e| MailSendError::Encoding(e))?;

                let bytes = encoder.to_vec()
                    .map_err(|e| MailSendError::Encoding(e))?;

                let body = Body::from(bytes);
                Ok(body)
            }).and_then(cloned!([service] => move |body| {
                let mut call_chain = Vec::new();

                let mailbox = mailbox2smtp_mailbox(envelop_data.sender());
                call_chain.push(service.call(Message::WithoutBody(SmtpRequest::Mail {
                    from: mailbox,
                    params: Vec::new()
                })));

                for to in envelop_data._to().iter() {
                    let mailbox = mailbox2smtp_mailbox(to);
                    call_chain.push(service.call(Message::WithoutBody(SmtpRequest::Rcpt {
                        to: mailbox,
                        params: Vec::new()
                    })));
                }

                future::join_all(call_chain)
                    .map_err(|io_error| MailSendError::Io(io_error))
                    .map(move |results| (results, body))

            })).and_then(cloned!([service] => move |(results, body)| {
                //this does not mean it was successful, just that there was no IOError
                let errors = results.into_iter()
                    .filter(|res| !res.code.severity.is_positive())
                    .collect::<Vec<_>>();

                if errors.is_empty() {
                    let fut = service
                        .call(Message::WithBody(SmtpRequest::Data, body))
                        .map_err(|e| MailSendError::Io(e))
                        .and_then(|response| {
                            if response.code.severity.is_positive() {
                                Ok(MailResponse)
                            } else {
                                Err(MailSendError::Smtp(vec![response]))
                            }
                        });
                    Either::A(fut)
                } else {
                    let fut = service
                        //TODO SmtpRequest::Reset does not exist
                        .call(Message::WithoutBody(SmtpRequest::Reset))
                        .map_err(|e| MailSendError::Io(e))
                        .and_then(|response| {
                            if response.code.severity.is_positive() {
                                Err(MailSendError::Smtp(errors))
                            } else {
                                Err(MailSendError::OnReset(response))
                            }
                        });
                    Either::B(fut)
                }
            })))
    }
}

#[derive(Debug)]
pub enum MailSendError {
    Composition(Error),
    Encoding(Error),
    Smtp(Vec<SmtpResponse>),
    Io(IoError),
    //Error returned if e.g. reset does not return ok or other strange thinks happen
    // and you need to reset the connection
    OnReset(SmtpResponse),
    DriverDropped,
    CanceledByDriver
}

fn mailbox2smtp_mailbox(mailbox: &Mailbox) -> SmtpMailbox {
    use emailaddress::EmailAddress;
    SmtpMailbox(Some(EmailAddress {
        local: mailbox.email.local_part.as_str().to_owned(),
        domain: mailbox.email.domain.as_str().to_owned(),
    }))
}

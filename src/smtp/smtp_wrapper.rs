use std::io::{Error as IoError};

use futures::future::{self, Either, Future};

use tokio_service::{Service as TokioService};

use tokio_smtp::request::{Request as SmtpRequest};
use tokio_smtp::response::{Response as SmtpResponse};
use tokio_proto::streaming::{Body, Message};

use super::error::MailSendError;
use super::common::{EnvelopData, MailResponse};


//
/////
///// # Example (Double Dispatch to allow multiple MailSendData Data types)
/////
///// ```
////TODO imports etc.
//// # type UserData = ();
//// # type OtherData = ();
//// enum Request {
////     UserData(UserData),
////     OtherData(OtherData)
//// }
////
//// impl MailSendRequest for Request {
////     fn call_compose_mail<CB>(&self, cb: &CB) -> Result<(Mail, EnvelopData), Error>
////         where CB: CompositionBase
////     {
////         use self::Request::*;
////         match *self {
////             UserData(ref data) => cb.compose_mail(data),
////             OtherData(ref data) => cb.compose_mail(data)
////         }
////     }
//// }
///// ```
/////
/////
//pub trait MailSendRequest {
//
//    /// Call the composition bases `compose_mail` method with the contained requests SendMailData
//    ///
//    /// If all teplates/send mail use the same type of Data this would just be implemented one the
//    /// data with `cb.compose_mail(self)` but if there are multiple different type for e.g.
//    /// different templates you can wrapp them in a enum implement `MailSendRequest` on the enum
//    /// and then match on the enum and calling `compose_mail` depending on the actual used type.
//    ///
//    fn call_compose_mail<CB>(&self, cb: &CB) -> Result<(Mail, EnvelopData), Error>
//        where CB: CompositionBase;
//}


pub(crate) fn send_mail<I: 'static>(
    service: &mut I,
    body_bytes: Vec<u8>,
    envelop: EnvelopData
) -> Box<Future<Item=MailResponse, Error=MailSendError>>
    where I: Clone + TokioService<
        Request=Message<SmtpRequest, Body<Vec<u8>, IoError>>,
        Response=SmtpResponse,
        Error=IoError
    >
{
    let (from, tos) = envelop.split();
    let body = Body::from(body_bytes);

    let mut call_chain = Vec::new();

    //TODO conversion should be done by the envelop type
    call_chain.push(service.call(Message::WithoutBody(SmtpRequest::Mail {
        from, params: Vec::new()
    })));

    for to in tos.into_iter() {
        call_chain.push(service.call(Message::WithoutBody(SmtpRequest::Rcpt {
            to, params: Vec::new()
        })));
    }

    let future = future::join_all(call_chain)
        .map_err(|io_error| MailSendError::Io(io_error))
        .and_then(cloned!([service] => move |results| {
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
        }));
    Box::new(future)
}


#[cfg(test)]
mod test {
    use std::io;
    use futures::{Future, IntoFuture};
    use super::super::service::TokioSmtpService;
    use super::super::test::*;
    use super::*;

    fn _test<F, R>(expected_requests: Vec<RequestMock>, use_response: Vec<ResponseMock>, func: F)
        where F: FnOnce(TokioSmtpService) -> R, R: IntoFuture<Item=(), Error=TestError>
    {
        let (proxy, driver) = FakeSmtpServer::new(expected_requests, use_response);
        let stop_flag = driver.get_stop_flag();

        let test_fut = func(proxy)
            .into_future()
            .then(|res| {
                stop_flag.stop();
                res
            });

        let res = driver.join(test_fut).wait();
        match res {
            Ok(_) => {},
            Err(TestError(msg)) => panic!(msg)
        }
    }


    #[test]
    fn send_simple_mail() {
        use self::RequestMock::*;
        let body: Vec<u8> = b"Not: a Mail".to_vec();

        let from_to = EnvelopData::new(
            "der@hund".parse().unwrap(),
            vec1![ "die@kat.ze".parse().unwrap(), "das@ze.bra".parse().unwrap()]
        );

        _test(vec![
            Normal(SmtpRequest::Mail { from: "der@hund".parse().unwrap(), params: Vec::new() }),
            Normal(SmtpRequest::Rcpt { to: "die@kat.ze".parse().unwrap(), params: Vec::new() }),
            Normal(SmtpRequest::Rcpt { to: "das@ze.bra".parse().unwrap(), params: Vec::new() }),
            Body(SmtpRequest::Data, body.clone())
        ], vec![
            Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
            Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
            Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
            Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
        ], |mut service| {
            send_mail(&mut service, body.clone(), from_to)
                .then(|res| match res {
                    Ok(MailResponse) => Ok(()),
                    Err(e) => Err(TestError(format!("err result: {:?}", e)))
                })
        })
    }

    #[test]
    fn bad_to() {
        use self::RequestMock::*;
        let body: Vec<u8> = b"Not: a Mail".to_vec();

        let from_to = EnvelopData::new(
            "der@hund".parse().unwrap(),
            vec1![ "die@kat.ze".parse().unwrap(), "das@ze.bra".parse().unwrap()]
        );

        let bad_response = SmtpResponse::parse(b"550 No such user here\r\n").unwrap().1;

        _test(vec![
            Normal(SmtpRequest::Mail { from: "der@hund".parse().unwrap(), params: Vec::new() }),
            Normal(SmtpRequest::Rcpt { to: "die@kat.ze".parse().unwrap(), params: Vec::new() }),
            Normal(SmtpRequest::Rcpt { to: "das@ze.bra".parse().unwrap(), params: Vec::new() }),
            Normal(SmtpRequest::Reset),
        ], vec![
            Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
            Ok(bad_response.clone()),
            // while the second failed we currently continue sending until befor data, which is fine
            // just not perfect
            Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
            Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
        ], |mut service| {
            send_mail(&mut service, body.clone(), from_to)
                .then(|res| match res {
                    Ok(MailResponse) => Err(TestError("unexpectadly no error".to_owned())),
                    Err(err) => {
                        if let MailSendError::Smtp(parts) = err {
                            if parts.len() == 1 && parts.first().unwrap() == &bad_response {
                                Ok(())
                            } else {
                                Err(TestError(format!("unexpected error kind {:?}",
                                                      MailSendError::Smtp(parts))))
                            }
                        } else {
                            Err(TestError(format!("unexpected error kind {:?}", err)))
                        }
                    },
                })
        })
    }

    #[test]
    fn bad_reset() {
        use self::RequestMock::*;
        let body: Vec<u8> = b"Not: a Mail".to_vec();

        let from_to = EnvelopData::new(
            "der@hund".parse().unwrap(),
            vec1![ "die@kat.ze".parse().unwrap()]
        );

        let bad_response = SmtpResponse::parse(b"550 No such user here\r\n").unwrap().1;
        let worse_response = SmtpResponse::parse(b"500 Server messed up\r\n").unwrap().1;
        _test(vec![
            Normal(SmtpRequest::Mail { from: "der@hund".parse().unwrap(), params: Vec::new() }),
            Normal(SmtpRequest::Rcpt { to: "die@kat.ze".parse().unwrap(), params: Vec::new() }),
            Normal(SmtpRequest::Reset),
        ], vec![
            Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
            Ok(bad_response.clone()),
            //NOTE: any standard compform smtp server must return 250 on RSET (but then what if..)
            Ok(worse_response.clone()),
        ], |mut service| {
            send_mail(&mut service, body.clone(), from_to)
                .then(|res| match res {
                    Ok(MailResponse) => Err(TestError("unexpectadly no error".to_owned())),
                    Err(err) => {
                        if let MailSendError::OnReset(resp) = err {
                            if resp == worse_response {
                                Ok(())
                            } else {
                                Err(TestError(format!("unexpected error kind {:?}",
                                                      MailSendError::OnReset(resp))))
                            }
                        } else {
                            Err(TestError(format!("unexpected error kind {:?}", err)))
                        }
                    },
                })
        })
    }

    #[test]
    fn io_failure() {
        use self::RequestMock::*;
        let body: Vec<u8> = b"Not: a Mail".to_vec();

        let from_to = EnvelopData::new(
            "der@hund".parse().unwrap(),
            vec1![ "die@kat.ze".parse().unwrap()]
        );


        _test(vec![
            Normal(SmtpRequest::Mail { from: "der@hund".parse().unwrap(), params: Vec::new() }),
            Normal(SmtpRequest::Rcpt { to: "die@kat.ze".parse().unwrap(), params: Vec::new() }),
            Normal(SmtpRequest::Reset),
        ], vec![
            Ok(SmtpResponse::parse(b"250 Ok\r\n").unwrap().1),
            Err(IoError::new(io::ErrorKind::Other, "it broke")),
        ], |mut service| {
            send_mail(&mut service, body.clone(), from_to)
                .then(|res| match res {
                    Ok(MailResponse) => Err(TestError("unexpectadly no error".to_owned())),
                    Err(err) => {
                        if let MailSendError::Io(_) = err {
                            Ok(())
                        } else {
                            Err(TestError(format!("unexpected error kind {:?}", err)))
                        }
                    },
                })
        })
    }
}
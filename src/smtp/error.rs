use std::io::{Error as IoError};
use ::error::Error;
use tokio_smtp::response::{Response as SmtpResponse};

#[derive(Debug)]
pub enum MailSendError {
    CreatingEnvelop(EnvelopFromMailError),
    Composition(Error),
    Encoding(Error),
    Smtp(Vec<SmtpResponse>),
    Io(IoError),
    //Error returned if e.g. reset does not return ok or other strange thinks happen
    // and you need to reset the connection
    /// A error value used if the client happens to detect that the server is "messed up"
    ///
    /// Currently this is only used when a server doesn't answer ok on a RSET cmd.
    ///
    /// Currently this causes the connection to be _dropped_. I.e. no Quit is send
    /// to the server (given that the server is also not standard conform and does
    /// error on RSET, this should be fine, while it's not perfect it's not worth
    /// the additional impl. cost for servers which are not RFC conform in a strange
    /// way, as there is no reason for a server to behave this way).
    OnReset(SmtpResponse),
    DriverDropped,
    CanceledByDriver
}


#[derive(Debug)]
pub enum EnvelopFromMailError {
    NeitherSenderNorFrom,
    TypeError(Error),
    NoSenderAndMoreThanOneFrom,
    NoToHeaderField
}

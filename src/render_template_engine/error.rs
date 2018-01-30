use std::{result, error};
use std::fmt::{self, Display};
use core::error::{Error as MailError};

pub type Result<T, E> = result::Result<T, Error<E>>;


#[derive(Debug)]
pub enum Error<RE: error::Error> {
    UnknownTemplateId(String),
    CIdGenFailed(MailError),
    RenderError(RE)
}


impl<R> error::Error for Error<R>
    where R: error::Error
{

    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            UnknownTemplateId(_) => "unknown template id",
            CIdGenFailed(_) => "generating a cid failed",
            RenderError(ref er) => er.description()
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        use self::Error::*;
        match *self {
            RenderError(ref er) => er.cause(),
            CIdGenFailed(ref er) => er.cause(),
            _ => None
        }
    }
}

impl<R> Display for Error<R>
    where R: error::Error
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;
        match *self {
            UnknownTemplateId(ref id) => {
                write!(fter, "unknwon template id: {:?}", id)
            },
            CIdGenFailed(ref err) => {
                write!(fter, "generating cid failed:")?;
                err.fmt(fter)
            }
            RenderError(ref re) => <R as fmt::Display>::fmt(re, fter)
        }
    }
}
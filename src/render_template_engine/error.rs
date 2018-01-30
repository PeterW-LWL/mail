use std::{result, error};
use std::fmt::{self, Display};
use std::path::PathBuf;

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


#[derive(Debug)]
pub enum SpecError {
    /// error if the path is not a valid string
    StringPath(PathBuf)
}

impl error::Error for SpecError {
    fn description(&self) -> &str {
        use self::SpecError::*;
        match *self {
            StringPath(_) => "path must also be valid string"
        }
    }
}

impl Display for SpecError {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        use self::SpecError::*;
        match *self {
            StringPath(ref path) => {
                write!(fter, "path must also be valid string, got: {}", path.display())
            }
        }
    }
}
use std::fmt::{self, Display};

use failure::{Fail, Backtrace};
use tera_crate;

#[derive(Debug)]
pub struct TeraError {
    kind: tera_crate::ErrorKind,
    backtrace: Backtrace
}

impl Fail for TeraError {

    fn cause(&self) -> Option<&Fail> {
        None
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        Some(&self.backtrace)
    }
}

impl Display for TeraError {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        use self::tera_crate::ErrorKind::*;

        match &self.kind {
            &Msg(ref msg) => write!(fter, "msg err: {}", msg),
            &Json(ref err) => write!(fter, "json err: {}", err),
            other => write!(fter, "unknown err: {:?}", other)
        }
    }
}

//TODO/BUG actually impl a real from
impl From<tera_crate::Error> for TeraError {
    fn from(err: tera_crate::Error) -> Self {
        let tera_crate::Error(kind, _state) = err;
        TeraError {
            kind,
            backtrace: Backtrace::new()
        }
    }
}
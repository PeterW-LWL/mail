use std::fmt::{self, Debug, Display};
use std::sync::Mutex;
use askama;

/// a wrapper needed as askama is not Sync
#[derive(Fail)]
pub struct AskamaError {
    inner: Mutex<askama::Error>
}

impl AskamaError {

    pub fn inner(&self) -> &Mutex<askama::Error> {
        &self.inner
    }
}

const POISON_MSG: &str = "<Debug/Display of crate::askama::Error paniced previously>";
macro_rules! impl_fmt {
    ($($trait:ident),*) => ($(
        impl $trait for AskamaError {
            fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
                match self.inner.lock() {
                    Ok(inner_error) => $trait::fmt(&*inner_error, fter),
                    Err(_err) => write!(fter, "{}", POISON_MSG)
                }
            }
        }
    )*);
}

impl_fmt!(Debug, Display);

impl From<askama::Error> for AskamaError {
    fn from(err: askama::Error) -> Self {
        AskamaError { inner: Mutex::new(err) }
    }
}
use std::fmt::{self, Display, Debug};
use std::mem::drop;

use failure::{Fail, Context, Backtrace};

use headers::error::{
    HeaderTypeError,
    ComponentCreationError
};
use mail::error::{BuilderError, OtherBuilderErrorKind};


/*
error cases:

- MailError
    - resource Loading
    - encoding
    - component creation
    - header validation

- Template
    - loading?
    - using?

*/

pub trait WithSourceExt:  Sized + Fail {
    fn with_source<S>(self, source: S) -> WithSource<Self, S>
        where S: Debug
    {
        WithSource::new(self, source)
    }
}

impl<FT> WithSourceExt for FT
    where FT: Fail
{}

/// Combines a error with a source into a type potentially implementing Fail
///
/// Fail is implemented if the Source if `Send`, `Sync` and `'static`.
/// The Display impl always forwards to the contained error as the
/// source might not implement `Display`
#[derive(Debug)]
pub struct WithSource<E: Fail, S: Debug> {
    error: E,
    source: S,
}

impl<E, S> WithSource<E, S>
    where E: Fail, S: Debug
{

    pub fn new(error: E, source: S) -> Self {
        WithSource { error, source }
    }
    pub fn source(&self) -> &S {
        &self.source
    }

    pub fn error(&self) -> &E {
        &self.error
    }

    pub fn into_source(self) -> S {
        let WithSource { error, source } = self;
        drop(error);
        source
    }

    pub fn into_error(self) -> E {
        let WithSource { error, source } = self;
        drop(source);
        error
    }

    pub fn split(self) -> (E, S) {
        let WithSource { error, source } = self;
        (error, source)
    }
}

impl<E, S> Fail for WithSource<E, S>
    where E: Fail, S: Debug + Send + Sync + 'static
{
    fn cause(&self) -> Option<&Fail> {
        self.error.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.error.backtrace()
    }
}

impl<E, S> Display for WithSource<E, S>
    where E: Fail, S: Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self.error(), fter)
    }
}

#[derive(Debug, Fail)]
pub enum CompositionError<TE: Fail> {
    #[fail(display = "{}", _0)]
    Template(TE),

    #[fail(display = "{}", _0)]
    Builder(ExtendedBuilderError)
}

impl<FT, TE> From<FT> for CompositionError<TE>
    where TE: Fail, ExtendedBuilderError: From<FT>
{
    fn from(error: FT) -> Self {
        CompositionError::Builder(error.into())
    }
}

// impl<TE> From<ExtendedBuilderError> for CompositionError<TE>
//     where TE: Fail
// {
//     fn from(error: ExtendedBuilderError) -> Self {
//         CompositionError::Builder(error)
//     }
// }

#[derive(Copy, Clone, Debug, Fail, PartialEq, Eq, Hash)]
pub enum ExtendedBuilderErrorKind {
    #[fail(display="need embedding to create a body with an embedding")]
    EmbeddingMissing,
}

#[derive(Debug, Fail)]
pub enum ExtendedBuilderError {
    #[fail(display = "{}", _0)]
    Normal(BuilderError),

    #[fail(display = "{}", _0)]
    Extended(Context<ExtendedBuilderErrorKind>)

}

//TODO[rust/bug?else specialization from]: use  wildcard for transitive impl
// impl<T> From<T> for ExtendedBuilderError
//     where BuilderError: From<T>
// {
//     fn from(err: T) -> Self {
//         let be = BuilderError::from(err);
//         ExtendedBuilderError::Normal(be)
//     }
// }

impl From<BuilderError> for ExtendedBuilderError {
    fn from(error: BuilderError) -> Self {
        ExtendedBuilderError::Normal(error)
    }
}
impl From<ExtendedBuilderErrorKind> for ExtendedBuilderError {
    fn from(err: ExtendedBuilderErrorKind) -> Self {
        ExtendedBuilderError::from(Context::new(err))
    }
}

impl From<Context<ExtendedBuilderErrorKind>> for ExtendedBuilderError {
    fn from(err: Context<ExtendedBuilderErrorKind>) -> Self {
        ExtendedBuilderError::Extended(err)
    }
}
impl From<OtherBuilderErrorKind> for ExtendedBuilderError {
    fn from(err: OtherBuilderErrorKind) -> Self {
        ExtendedBuilderError::Normal(err.into())
    }
}

impl From<Context<OtherBuilderErrorKind>> for ExtendedBuilderError {
    fn from(err: Context<OtherBuilderErrorKind>) -> Self {
        ExtendedBuilderError::Normal(err.into())
    }
}

impl From<HeaderTypeError> for ExtendedBuilderError {
    fn from(err: HeaderTypeError) -> Self {
        ExtendedBuilderError::Normal(err.into())
    }
}

impl From<ComponentCreationError> for ExtendedBuilderError {
    fn from(err: ComponentCreationError) -> Self {
        ExtendedBuilderError::Normal(err.into())
    }
}

#[derive(Copy, Clone, Debug, Fail, PartialEq, Eq, Hash)]
pub enum MailSendDataErrorKind {
    #[fail(display = "missing data for From field")]
    MissingFrom,

    #[fail(display = "missing data for To field")]
    MissingTo,

    #[fail(display = "missing data for Subject field")]
    MissingSubject,

    #[fail(display = "missing template id")]
    MissingTemplateId,

    #[fail(display = "missing template data")]
    MissingTemplateData,

    #[fail(display = "multiple mailboxes in from field but no sender field")]
    MultiFromButNoSender
}

#[derive(Debug)]
pub struct MailSendDataError {
    inner: Context<MailSendDataErrorKind>,
}

impl Fail for MailSendDataError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for MailSendDataError {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, fter)
    }
}

impl From<MailSendDataErrorKind> for MailSendDataError {
    fn from(kind: MailSendDataErrorKind) -> Self {
        MailSendDataError::from(Context::new(kind))
    }
}

impl From<Context<MailSendDataErrorKind>> for MailSendDataError {
    fn from(inner: Context<MailSendDataErrorKind>) -> Self {
        MailSendDataError { inner }
    }
}
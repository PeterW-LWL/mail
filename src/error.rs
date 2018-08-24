//! Module contains all custom errors introduced by this crate.
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

/// Extension adding a  `with_source` method to anything implementing `Fail`.
///
/// This provides a uniform way to combine a error with the source which
/// caused the error and return them together. The returned `WithSource`
/// instance will implement `Fail` if possible but does not require it,
/// making it compatible with non 'static, Send, Sync sources.
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

    /// Create a new instance given an error and it's source.
    pub fn new(error: E, source: S) -> Self {
        WithSource { error, source }
    }

    /// Return a reference to it's source.
    pub fn source(&self) -> &S {
        &self.source
    }

    /// Return a reference to the contained error.
    pub fn error(&self) -> &E {
        &self.error
    }

    /// Turns this type into it's source.
    pub fn into_source(self) -> S {
        let WithSource { error, source } = self;
        drop(error);
        source
    }

    /// Turns this type into it's error.
    pub fn into_error(self) -> E {
        let WithSource { error, source } = self;
        drop(source);
        error
    }

    /// Decomposes this type into its error and its source.
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

/// Error returned when composing a `Mail` failed.
#[derive(Debug, Fail)]
pub enum CompositionError<TE: Fail> {
    /// It failed to use the underlying template engine.
    #[fail(display = "{}", _0)]
    Template(TE),

    /// It didn't fail to get all `MailParts` but wasn't
    /// able to compose them into an mail.
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

/// Kinds of Error which can be caused by the builder extension.
///
/// (The builder extension is a trait providing additional methods
///  to the `MailBuilder`)
#[derive(Copy, Clone, Debug, Fail, PartialEq, Eq, Hash)]
pub enum ExtendedBuilderErrorKind {
    #[fail(display="need embedding to create a body with an embedding")]
    EmbeddingMissing,
}

/// Error returned if building the mail failed.
#[derive(Debug, Fail)]
pub enum ExtendedBuilderError {
    /// A error covered by `BuilderError` occurred.
    #[fail(display = "{}", _0)]
    Normal(BuilderError),

    /// An error not covered by `BuilderError` occurred.
    #[fail(display = "{}", _0)]
    Extended(Context<ExtendedBuilderErrorKind>)

}

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

/// Error kinds associated with creating `MailSendData`
#[derive(Copy, Clone, Debug, Fail, PartialEq, Eq, Hash)]
pub enum MailSendDataErrorKind {
    /// No `From` was set.
    #[fail(display = "missing data for From field")]
    MissingFrom,

    /// No `To` was set.
    #[fail(display = "missing data for To field")]
    MissingTo,

    /// No `Subject` was set.
    #[fail(display = "missing data for Subject field")]
    MissingSubject,

    /// No `TemplateId` was given.
    #[fail(display = "missing template id")]
    MissingTemplateId,

    /// No `TemplateData` was given.
    #[fail(display = "missing template data")]
    MissingTemplateData,

    /// No `Sender` was given in an situation where it's needed.
    #[fail(display = "multiple mailboxes in from field but no sender field")]
    MultiFromButNoSender
}

/// Error returned when building `MailSendData` with the `MailSendDataBuilder` failed.
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

use mail::error::{CompositionError, ComponentCreationError};
use mail::render_template_engine::error::{CreatingSpecError, InsertionError as _InsertionError};
use mail::tera::error::TeraError;

type InsertionError = _InsertionError<TeraError>;

#[derive(Fail, Debug)]
pub enum SetupError {
    #[fail(display = "{}", _0)]
    Tera(TeraError),

    #[fail(display = "{}", _0)]
    CreatingSpecs(CreatingSpecError),

    #[fail(display = "{}", _0)]
    UsingSpecs(InsertionError)
}

impl From<TeraError> for SetupError {
    fn from(err: TeraError) -> Self {
        SetupError::Tera(err)
    }
}
impl From<InsertionError> for SetupError {
    fn from(err: InsertionError) -> Self {
        SetupError::UsingSpecs(err)
    }
}

impl From<CreatingSpecError> for SetupError {
    fn from(err: CreatingSpecError) -> Self {
        SetupError::CreatingSpecs(err)
    }
}

// TODO `Header` should be mergeable into `Composition`.
#[derive(Fail, Debug)]
pub enum Error {
    #[fail(display = "{}", _0)]
    Composition(CompositionError<TeraError>),
    #[fail(display = "{}", _0)]
    Header(ComponentCreationError)
}

impl From<CompositionError<TeraError>> for Error {
    fn from(err: CompositionError<TeraError>) -> Self {
        Error::Composition(err)
    }
}

impl From<ComponentCreationError> for Error {
    fn from(err: ComponentCreationError) -> Self {
        Error::Header(err)
    }
}

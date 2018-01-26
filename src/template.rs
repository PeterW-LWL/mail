use std::result::{ Result as StdResult };
use std::error::{ Error as StdError };

use serde::Serialize;

use vec1::Vec1;
use mail::Resource;

use resource::{Embedding, Attachment};
use context::Context;

///
/// # Why is Context a generic of the Type?
///
/// a context independent template engine can be simple implemented
/// with `impl<C: Context> TemplateEngine<C> for TheEngine` the reason
/// why `C` is not part of the `TemplateEngine::templates` function is
/// so that a template engine can depend on a specifc context type.
///
/// Such a context type could, for example, provide access to the
/// current server configuration, preventing the need for the
/// template engine to store a handle to it/copy of it itself.
pub trait TemplateEngine<C: Context> {
    type TemplateId;
    type Error: StdError + Send + 'static;

    fn templates<D: Serialize>( &self,  ctx: &C, id: Self::TemplateId, data: D )
                                -> StdResult< Vec1<Template>, Self::Error >;
}


pub struct Template {
    pub body: Resource,
    pub embeddings: Vec<Embedding>,
    pub attachments: Vec<Attachment>
}
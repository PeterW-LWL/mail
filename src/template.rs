use std::collections::HashMap;
use std::result::{ Result as StdResult };
use std::error::{ Error as StdError };

use serde::Serialize;

use vec1::Vec1;
use mail::Resource;

use resource::{EmbeddingWithCId, Attachment};
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
    type TemplateId: ?Sized;
    type Error: StdError + Send + 'static;

    fn templates<D: Serialize>(
        &self,  ctx: &C, id: &Self::TemplateId, data: D
    ) -> StdResult<(Vec1<TemplateBody>, Vec<Attachment>), Self::Error >;
}

//TODO move this to BuilderExt and just use it here (oh and rename it)
/// A mail body created by a template engine
pub struct TemplateBody {
    /// a body created by a template
    pub body_resource: Resource,
    /// embeddings added by the template engine
    ///
    /// It is a mapping of the name under which a embedding had been made available in the
    /// template engine to the embedding (which has to contain a CId, as it already
    /// was used in the template engine and CIds are used to link to the content which should
    /// be embedded)
    pub embeddings: HashMap<String, EmbeddingWithCId>,

}
use std::ops::Deref;
use std::sync::Arc;
use std::rc::Rc;

use failure::Fail;
use vec1::Vec1;

use mail::Context;

use ::resource::{EmbeddedWithCId, InspectEmbeddedResources};
use ::builder_extension::BodyPart;

/// This trait needs to be implemented for creating mails based on templates.
///
/// As there are many ways to have templates and many different approaches
/// on how to pass data to the template this crate doesn't impement the
/// full mail template stack instead it focuses on creating a mail from
/// the parts produced by the template engine and delegates the work of
/// producing such parts to the implementation of this trait.
///
/// The work flow is roughly as following (skipping over
/// the `Context` as it's for providing a thread pool and
/// id generation):
///
//TODO[NOW] workflow doesn't seem healthy
///
/// - have a template engine
/// - get contextual information like sender, recipient
/// - get template id and template data
/// - use that to generate `MailSendData`
/// - call the method to generate a mail which
///   - will call `TemplateEngine::use_template(id, data, ..)`
///   - use the returned mail parts to generate a `Mail` instance
///   - return the `Mail` instance as result
///
/// # Implementations
///
/// There is a default implementation using the `askama` template
/// engine which can be made available with the `askama-engine` feature,
/// but is limited in usefulness due to the way askama works.
///
/// Additionally there is the `mail-render-template-engine` which provides
/// a implementation just missing some simple text template engine which
/// as default bindings for handlebars (behind a feature flag).
///
///
/// # Why is Context a generic of the Type?
///
/// a context independent template engine can be simple implemented
/// with `impl<C: Context> TemplateEngine<C> for TheEngine` the reason
/// why `C` is not part of the `TemplateEngine::templates` function is
/// so that a template engine can depend on a specific context type.
///
/// Such a context type could, for example, provide access to the
/// current server configuration, preventing the need for the
/// template engine to store a handle to it/copy of it itself.
///
///
/// # Why is data a generic of the type?
///
/// Many template engine in the rust ecosystem use serialization
/// to access the data. Nevertheless there are a view like askama
/// which use a different approach only by being generic in the
/// trait over the data type can we support all of them.
pub trait TemplateEngine<C, D>
    where C: Context
{
    /// The type used for template ids.
    ///
    /// Normally this will be `str`.
    type TemplateId: ?Sized + ToOwned;

    /// The error type returned by the template engine.
    type Error: Fail;

    fn use_template(
        &self,
        id: &Self::TemplateId,
        data: &D,
        ctx: &C
    ) -> Result<MailParts, Self::Error>;
}

/// Parts which can be used to compose a multipart mail.
///
/// Instances of this type are produced by the implementor of the
/// `TemplateEngine` trait and consumed by this crate to generate
/// a `Mail` instance.
///
//TODO[LATER]
/// # Current Limitations
///
/// Currently there is no way to pass in `MailParts` from an external
/// point to produce a `Mail` instance. I.e. the only way they are
/// used is if `MailSendData` is used to create a mail and the procedure
/// calls the `TemplateEngine` to get it's mail parts. This might change
/// in future versions.
pub struct MailParts {
    /// A vector of alternate bodies
    ///
    /// A typical setup would be to have two alternate bodies one text/html and
    /// another text/plain as fallback (for which the text/plain body would be
    /// the first in the vec and the text/html body the last one).
    ///
    /// Note that the order in the vector     /// a additional text/plainis
    /// the same as the order in which they will appear in the mail. I.e.
    /// the first one is the last fallback while the last one should be
    /// shown if possible.
    pub alternative_bodies: Vec1<BodyPart>,

    /// Embeddings shared between alternative_bodies.
    ///
    /// Any resource in there can be referenced to by all
    /// alternative bodies via CId.
    pub shared_embeddings: Vec<EmbeddedWithCId>,

    /// Resources added to the mail as attachments.
    pub attachments: Vec<EmbeddedWithCId>
}

macro_rules! impl_for_1elem_container {
    ($($name:ident),*) => ($(
        impl<C, D, T> TemplateEngine<C, D> for $name<T>
            where T: TemplateEngine<C, D>,
                C: Context, D: InspectEmbeddedResources
        {
            type TemplateId = T::TemplateId;
            type Error = T::Error;

            fn use_template(
                &self,
                id: &Self::TemplateId,
                data: &D,
                ctx: &C
            ) -> Result<MailParts, Self::Error>
            {
                self.deref().use_template(id, data, ctx)
            }
        }
    )*);
}

impl_for_1elem_container! {
    Box,
    Arc,
    Rc
}

macro_rules! impl_for_1elem_ref {
    ($([$ref0:tt $($ref1:tt)*]),*) => ($(
        impl<'a, C, D, T> TemplateEngine<C, D> for $ref0 'a $($ref1)* T
            where T: TemplateEngine<C, D>,
                C: Context, D: InspectEmbeddedResources
        {
            type TemplateId = T::TemplateId;
            type Error = T::Error;

            fn use_template(
                &self,
                id: &Self::TemplateId,
                data: &D,
                ctx: &C
            ) -> Result<MailParts, Self::Error>
            {
                self.deref().use_template(id, data, ctx)
            }
        }
    )*);
}

impl_for_1elem_ref! {
    [&], [&mut]
}

//TODO[maybe] if a `use_parking_lot` feature is included provide parking lot RwLock/Mutex wrapper impl. too


#[cfg(test)]
mod test {

    mod TemplateEngine {
        #![allow(non_snake_case)]

        use std::sync::Arc;
        use mail::Context;
        use super::super::TemplateEngine;
        use ::resource::InspectEmbeddedResources;

        //just a compiler time type check
        fn _auto_impl_for_arc_and_box<C, D, T>(dumy: Option<T>)
            where T: TemplateEngine<C, D>, C: Context, D: InspectEmbeddedResources
        {
            if dumy.is_some() {
                _auto_impl_for_arc_and_box(dumy.map(|te| Arc::new(Box::new(te))))
            }
        }

    }
}
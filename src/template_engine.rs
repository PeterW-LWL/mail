use std::ops::Deref;
use std::sync::Arc;
use std::rc::Rc;

use failure::Fail;
use vec1::Vec1;

use mail::Context;

use ::resource::{EmbeddedWithCId, InspectEmbeddedResources};
use ::builder_extension::BodyPart;

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
pub trait TemplateEngine<C, D>
    where C: Context
{
    type TemplateId: ?Sized + ToOwned;
    type Error: Fail;

    fn use_template(
        &self,
        id: &Self::TemplateId,
        data: &D,
        ctx: &C
    ) -> Result<MailParts, Self::Error>;
}


pub struct MailParts {
    pub alternative_bodies: Vec1<BodyPart>,
    /// embeddings shared between alternative_bodies
    pub shared_embeddings: Vec<EmbeddedWithCId>,
    pub attachments: Vec<EmbeddedWithCId>
}

//TODO move this to BuilderExt and just use it here (oh and rename it)


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
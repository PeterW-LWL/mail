use std::ops::Deref;

use mail::Context;
use headers::components::ContentId;
use mail::Resource;
#[cfg(feature="serialize-content-id")]
use serde::{Serialize, Serializer, ser};

pub use headers::components::DispositionKind as Disposition;

mod impl_inspect;

/// # Serialize (feature `serialize-content-id`)
///
/// If serialized this struct **turns into it's
/// content id failing if it has no content id**.
///
/// Normally this struct would not be serializeable
/// (Resource isn't) but for template engines which
/// use serialization for data access serializing it
/// to it's content id string is quite use full
#[derive(Debug, Clone)]
pub struct Embedded {
    content_id: Option<ContentId>,
    resource: Resource,
    disposition: Disposition,
}

impl Embedded {
    pub fn inline(resource: Resource) -> Self {
        Embedded::new(resource, Disposition::Inline)
    }

    pub fn attachment(resource: Resource) -> Self {
        Embedded::new(resource, Disposition::Attachment)
    }

    pub fn new(resource: Resource, disposition: Disposition) -> Self {
        Embedded {
            content_id: None,
            resource,
            disposition
        }
    }

    pub fn with_content_id(resource: Resource, disposition: Disposition, content_id: ContentId) -> Self {
        Embedded {
            content_id: Some(content_id),
            resource,
            disposition
        }
    }

    pub fn resource(&self) -> &Resource {
        &self.resource
    }

    pub fn resource_mut(&mut self) -> &mut Resource {
        &mut self.resource
    }

    pub fn content_id(&self) -> Option<&ContentId> {
        self.content_id.as_ref()
    }

    pub fn disposition(&self) -> Disposition {
        self.disposition
    }

    pub fn assure_content_id(&mut self, ctx: &impl Context) -> &ContentId {
        if self.content_id.is_none() {
            self.content_id = Some(ctx.generate_content_id());
        }

        self.content_id().unwrap()
    }

    /// Generate and set a new content id if needed and additionally clone the type into an `EmbeddedWithCId` instance.
    ///
    /// Given that `Resource` instances are meant to be cheap to clone this should not be very
    /// expansive (at last if no new content is generated).
    pub fn assure_content_id_and_copy(&mut self, ctx: &impl Context) -> EmbeddedWithCId {
        self.assure_content_id(ctx);
        EmbeddedWithCId { inner: self.clone() }
    }
}

#[cfg(feature="serialize-to-content-id")]
impl<'a> Serialize for Embedded {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        if let Some(cid) = self.content_id() {
            cid.serialize(serializer)
        } else {
            Err(ser::Error::custom("can not serialize Embedded without content id"))
        }
    }
}

/// This trait is used to iterate over all `Embedded` instances of "arbitrary" data.
///
/// This functionality is needed so that the Content-Id's for embedded resources
/// and attachments can be retrieved and generated for any kind of data a user might
/// pass to any kind of template engine.
///
/// This trait is implemented for many types from `std`, this include two kind
/// of implementations:
///
/// 1. such for containers e.g. `Vec<T> where T: InspectEmbeddedResources`
/// 2. such for values which definitely can _not_ contain a `Embedded` type,
///    e.g. u32 or String (currently not all types of `std` for which this is
///    the case have this "empty" implementation, more can/will be added)
///
/// For types which can contain a `Embedded` but accessing it `&mut` might
/// not be possible not implementation is provided intentionally. This includes
/// `Rc<T>`, `Mutex<T>` (might be locked/poisoned), etc. When specialization
/// is stable this might be possible extended to include more cases where
/// a only a "empty" implementation makes sense e.g. `Rc<u32>`.
///
/// (note a "empty" implementation simple does nothing when called, which
///  is fine and more or less like not calling it at all in this case)
///
/// # Derive
///
/// There is a custom derive for this type which can
/// be used as shown in the example below. It tries
/// to call the `inspect_resources*` methods on every
/// field and considers following (field) attributes:
///
/// - `#[mail(inspect_skip)]`
///   skip field, does not call `inspect_resource*` on it
/// - `#[mail(inspect_with="(some::path, another::path_mut)")]`
///   use the functions `some::path` and `another::path_mut` to inspect
///   the field (using the former for `inspect_resources` and the later
///   for `inspect_resources_mut`)
///
/// The derive works for struct and enum types in all variations but not
/// unions.
///
/// ```
/// # #[macro_use]
/// # extern crate mail_template;
/// # use std::sync::{Mutex, Arc};
///
/// // due to some current limitations of rust the macro
/// // can not import the types, so we need to do so
/// use mail_template::{Embedded, InspectEmbeddedResources};
///
/// struct Bloop {
///     //...
/// # bla: u32
/// }
///
/// #[derive(InspectEmbeddedResources)]
/// struct Foo {
///     // has an "empty" implementation
///     field1: u32,
///
///     // skips this field
///     #[mail(inspect_skip)]
///     field2: Bloop,
///
///     // inspects this field
///     dings: Vec<Embedded>,
///
///     // custom inspection handling (2 pathes to functions)
///     #[mail(inspect_with="(inspect_mutex, inspect_mutex_mut)")]
///     shared_dings: Arc<Mutex<Embedded>>
/// }
///
/// // due too auto-deref we could use `me: &Mutex<Embedded>`
/// fn inspect_mutex(me: &Arc<Mutex<Embedded>>, visitor: &mut FnMut(&Embedded)) {
///     let embedded = me.lock().unwrap();
///     visitor(&*embedded);
/// }
///
/// fn inspect_mutex_mut(me: &mut Arc<Mutex<Embedded>>, visitor: &mut FnMut(&mut Embedded)) {
///     let mut embedded = me.lock().unwrap();
///     visitor(&mut *embedded);
/// }
///
/// # fn main() {}
/// ```
pub trait InspectEmbeddedResources {
    fn inspect_resources(&self, visitor: &mut FnMut(&Embedded));
    fn inspect_resources_mut(&mut self, visitor: &mut FnMut(&mut Embedded));
}


impl InspectEmbeddedResources for Embedded {
    fn inspect_resources(&self, visitor: &mut FnMut(&Embedded)) {
        visitor(self)
    }
    fn inspect_resources_mut(&mut self, visitor: &mut FnMut(&mut Embedded)) {
        visitor(self)
    }
}

impl Into<Resource> for Embedded {
    fn into(self) -> Resource {
        let Embedded { content_id:_, resource, disposition:_ } = self;
        resource
    }
}


/// # Serialize (feature `serialize-content-id`)
///
/// If serialized this struct **turns into it's
/// content id**.
///
/// Normally this struct would not be serializeable
/// (Resource isn't) but for template engines which
/// use serialization for data access serializing it
/// to it's content id string is quite use full
#[derive(Debug, Clone)]
pub struct EmbeddedWithCId {
    inner: Embedded
}

impl Deref for EmbeddedWithCId {
    type Target = Embedded;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl EmbeddedWithCId {

    /// create a new embedding with an inline disposition
    ///
    /// The context is used to generate a fitting content id.
    pub fn inline(resource: Resource, ctx: &impl Context) -> Self {
        EmbeddedWithCId::new(resource, Disposition::Inline, ctx)
    }

    /// create a new embedding with an attachment disposition
    ///
    /// The context is used to generate a fitting content id.
    pub fn attachment(resource: Resource, ctx: &impl Context) -> Self {
        EmbeddedWithCId::new(resource, Disposition::Attachment, ctx)
    }

    /// create a new embedding
    ///
    /// The context is used to generate a fitting content id.
    pub fn new(resource: Resource, disposition: Disposition, ctx: &impl Context) -> Self {
        EmbeddedWithCId {
            inner: Embedded::with_content_id(resource, disposition, ctx.generate_content_id())
        }
    }

    /// Tries to convert an `Embedded` instance to an `EmbeddedWithCId` instance.
    ///
    /// # Error
    ///
    /// If the `Embedded` instance doesn't have a content id the passed in
    /// `Embedded` instance is returned as error.
    pub fn try_from(emb: Embedded) -> Result<EmbeddedWithCId, Embedded> {
        if emb.content_id().is_some() {
            Ok(EmbeddedWithCId { inner: emb })
        } else {
            Err(emb)
        }
    }

    /// return the content id
    pub fn content_id(&self) -> &ContentId {
        self.inner.content_id().unwrap()
    }
}

#[cfg(feature="serialize-to-content-id")]
impl<'a> Serialize for EmbeddedWithCId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        self.content_id().serialize(serializer)
    }
}
impl Into<Resource> for EmbeddedWithCId {
    fn into(self) -> Resource {
        let EmbeddedWithCId { inner } = self;
        let Embedded { content_id:_, resource, disposition:_ } = inner;
        resource
    }
}

impl Into<(ContentId, Resource)> for EmbeddedWithCId {

    fn into(self) -> (ContentId, Resource) {
        let EmbeddedWithCId { inner } = self;
        let Embedded { content_id, resource, disposition:_ } = inner;
        (content_id.unwrap(), resource)
    }
}




#[cfg(test)]
mod test {
    use soft_ascii_string::SoftAsciiString;
    use mail::{Context, Resource};
    use mail::default_impl::simple_context;
    use headers::components::{ContentId, Domain};
    use headers::HeaderTryFrom;

    use ::resource::Disposition;

    fn any_resource() -> Resource {
        Resource::sourceless_from_string("abc")
    }

    fn ctx() -> impl Context {
        simple_context::new(
            Domain::try_from("hy.test").unwrap(),
            SoftAsciiString::from_unchecked("9ddqdq")
        ).unwrap()
    }

    mod Embedded {
        #![allow(non_snake_case)]

        use super::*;
        use super::super::Embedded;

        #[test]
        fn inline_uses_disposition_inline() {
            let emb = Embedded::inline(any_resource());

            assert_eq!(emb.disposition(), Disposition::Inline)
        }

        #[test]
        fn attachment_uses_disposition_attachment() {
            let emb = Embedded::attachment(any_resource());

            assert_eq!(emb.disposition(), Disposition::Attachment)
        }

        #[test]
        fn assure_content_id_create_a_content_id() {
            let ctx = ctx();
            let mut emb = Embedded::inline(any_resource());
            assert_eq!(emb.content_id(), None);

            emb.assure_content_id(&ctx);
            assert!(emb.content_id().is_some());
        }

        #[test]
        fn assure_content_id_create_a_content_id_only_if_needed() {
            let ctx = ctx();
            let mut emb = Embedded::inline(any_resource());
            assert_eq!(emb.content_id(), None);

            emb.assure_content_id(&ctx);

            let cid = emb
                .content_id()
                .expect("content id should have been generated")
                .clone();

            emb.assure_content_id(&ctx);
            assert_eq!(emb.content_id(), Some(&cid));
        }
    }

    mod EmbeddedWithCId {
        #![allow(non_snake_case)]
        use super::*;
        use super::super::{Embedded, EmbeddedWithCId};

        #[test]
        fn generates_a_cid() {
            let ctx = ctx();

            let emb_wcid = EmbeddedWithCId::inline(any_resource(), &ctx);
            let emb: &Embedded = &emb_wcid;
            assert!(emb.content_id().is_some());
            let _: &ContentId = emb_wcid.content_id();
        }
    }
}
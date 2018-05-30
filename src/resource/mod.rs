use std::ops::Deref;

use mail::Context;
use headers::components::ContentId;
use mail::Resource;

pub use headers::components::DispositionKind as Disposition;

mod impl_inspect;

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

    pub fn assure_content_id_and_copy(&mut self, ctx: &impl Context) -> EmbeddedWithCId {
        self.assure_content_id(ctx);
        EmbeddedWithCId { inner: self.clone() }
    }
}

pub trait InspectEmbeddedResources {
    fn inspect_resources(&self, visitor: &mut impl FnMut(&Embedded));
    fn inspect_resources_mut(&mut self, visitor: &mut impl FnMut(&mut Embedded));
}


impl InspectEmbeddedResources for Embedded {
    fn inspect_resources(&self, visitor: &mut impl FnMut(&Embedded)) {
        visitor(self)
    }
    fn inspect_resources_mut(&mut self, visitor: &mut impl FnMut(&mut Embedded)) {
        visitor(self)
    }
}

impl Into<Resource> for Embedded {
    fn into(self) -> Resource {
        let Embedded { content_id:_, resource, disposition:_ } = self;
        resource
    }
}


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

    pub fn inline(resource: Resource, ctx: &impl Context) -> Self {
        EmbeddedWithCId::new(resource, Disposition::Inline, ctx)
    }

    pub fn attachment(resource: Resource, ctx: &impl Context) -> Self {
        EmbeddedWithCId::new(resource, Disposition::Attachment, ctx)
    }

    pub fn new(resource: Resource, disposition: Disposition, ctx: &impl Context) -> Self {
        EmbeddedWithCId {
            inner: Embedded::with_content_id(resource, disposition, ctx.generate_content_id())
        }
    }

    pub fn try_from(emb: Embedded) -> Result<EmbeddedWithCId, Embedded> {
        if emb.content_id().is_some() {
            Ok(EmbeddedWithCId { inner: emb })
        } else {
            Err(emb)
        }
    }

    pub fn content_id(&self) -> &ContentId {
        self.inner.content_id().unwrap()
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
            SoftAsciiString::from_string_unchecked("9ddqdq")
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
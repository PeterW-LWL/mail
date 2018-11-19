extern crate failure;
extern crate serde;
extern crate futures;
extern crate galemu;
extern crate mail_core;
extern crate mail_headers;
extern crate vec1;
extern crate toml;

use std::{
    fs,
    collections::HashMap,
    fmt::Debug,
    path::{PathBuf},
    sync::Arc
};

use serde::{
    Serialize,
    Deserialize
};
use galemu::{Bound, BoundExt};
use failure::{Fail, Error};
use futures::{
    Future, Poll, Async,
    try_ready,
    future::{self, Join, Either}
};
use vec1::Vec1;

use mail_core::{
    Resource,
    Data, Metadata,
    Context, ResourceContainerLoadingFuture,
    compose::{MailParts, BodyPart},
    Mail
};
use mail_headers::{
    HeaderKind, Header,
    header_components::MediaType,
    headers
};

pub mod serde_impl;
mod base_dir;
mod path_rebase;
mod additional_cid;

pub use self::base_dir::*;
pub use self::path_rebase::*;
pub use self::additional_cid::*;

pub trait TemplateEngine: Sized {
    type Id: Debug;
    type Error: Fail;

    type LazyBodyTemplate: PathRebaseable + Debug + Send + Serialize + for<'a> Deserialize<'a>;

    fn load_body_template(&mut self, tmpl: Self::LazyBodyTemplate)
        -> Result<BodyTemplate<Self>, Error>;

    fn load_subject_template(&mut self, template_string: String)
        -> Result<Self::Id, Error>;
}

pub fn load_toml_template_from_path<TE, C>(
    engine: TE,
    path: PathBuf,
    ctx: &C
) -> impl Future<Item=Template<TE>, Error=Error>
    where TE: TemplateEngine + 'static, C: Context
{

    let ctx2 = ctx.clone();
    ctx.offload_fn(move || {
        let content = fs::read_to_string(path)?;
        let base: serde_impl::TemplateBase<TE> = toml::from_str(&content)?;
        Ok(base)
    }).and_then(move |base| base.load(engine, &ctx2))
}

pub fn load_toml_template_from_str<TE, C>(
    engine: TE,
    content: &str,
    ctx: &C
) -> impl Future<Item=Template<TE>, Error=Error>
    where TE: TemplateEngine, C: Context
{
    let base: serde_impl::TemplateBase<TE> =
        match toml::from_str(content) {
            Ok(base) => base,
            Err(err) => { return Either::B(future::err(Error::from(err))); }
        };

    Either::A(base.load(engine, ctx))
}

pub struct PreparationData<'a, PD: for<'any> BoundExt<'any>> {
    pub attachments: Vec<Resource>,
    pub inline_embeddings: HashMap<String, Resource>,
    pub prepared_data: Bound<'a, PD>
}

pub trait UseTemplateEngine<D>: TemplateEngine {

    //TODO[doc]: this is needed for all template engines which use to json serialization
    // (we have more then one template so there would be a lot of overhead)
    type PreparedData: for<'a> BoundExt<'a>;

    //TODO[design]: allow returning a result
    fn prepare_data<'a>(&self, raw: &'a D) -> PreparationData<'a, Self::PreparedData>;

    fn render<'r, 'a>(
        &'r self,
        id: &'r Self::Id,
        data: &'r Bound<'a, Self::PreparedData>,
        additional_cids: AdditionalCIds<'r>
    ) -> Result<String, Self::Error>;
}

#[derive(Debug)]
pub struct Template<TE: TemplateEngine> {
    inner: Arc<InnerTemplate<TE>>
}

impl<TE> Template<TE>
    where TE: TemplateEngine
{
    pub fn inline_embeddings(&self) -> &HashMap<String, Resource> {
        &self.inner.embeddings
    }

    pub fn attachments(&self) -> &[Resource] {
        &self.inner.attachments
    }

    pub fn engine(&self) -> &TE {
        &self.inner.engine
    }

    pub fn bodies(&self) -> &[BodyTemplate<TE>] {
        &self.inner.bodies
    }

    pub fn subject_template_id(&self) -> &TE::Id {
        &self.inner.subject.template_id
    }
}

impl<TE> Clone for Template<TE>
    where TE: TemplateEngine
{
    fn clone(&self) -> Self {
        Template { inner: self.inner.clone() }
    }
}

#[derive(Debug)]
struct InnerTemplate<TE: TemplateEngine> {
    template_name: String,
    base_dir: CwdBaseDir,
    subject: Subject<TE>,
    /// This can only be in the loaded form _iff_ this is coupled
    /// with a template engine instance, as using it with the wrong
    /// template engine will lead to potential bugs and panics.
    bodies: Vec1<BodyTemplate<TE>>,
    //TODO: make sure
    embeddings: HashMap<String, Resource>,
    attachments: Vec<Resource>,
    engine: TE,
}


pub trait TemplateExt<TE, D>
    where TE: TemplateEngine + UseTemplateEngine<D>
{
    fn prepare_to_render<'s, 'r, C>(&'s self, data: &'r D, ctx: &'s C) -> RenderPreparationFuture<'r, TE, D, C>
        where C: Context;
}


impl<TE, D> TemplateExt<TE, D> for Template<TE>
    where TE: TemplateEngine + UseTemplateEngine<D>
{
    fn prepare_to_render<'s, 'r, C>(&'s self, data: &'r D, ctx: &'s C) -> RenderPreparationFuture<'r, TE, D, C>
        where C: Context
    {
        let preps = self.inner.engine.prepare_data(data);

        let PreparationData {
            inline_embeddings,
            attachments,
            prepared_data
        } = preps;

        let loading_fut = Resource::load_container(inline_embeddings, ctx)
            .join(Resource::load_container(attachments, ctx));

        RenderPreparationFuture {
            payload: Some((
                self.clone(),
                prepared_data,
                ctx.clone()
            )),
            loading_fut
        }
    }
}

pub struct RenderPreparationFuture<'a, TE, D, C>
    where TE: TemplateEngine + UseTemplateEngine<D>, C: Context
{
    payload: Option<(
        Template<TE>,
        Bound<'a, <TE as UseTemplateEngine<D>>::PreparedData>,
        C
    )>,
    loading_fut: Join<
        ResourceContainerLoadingFuture<HashMap<String, Resource>>,
        ResourceContainerLoadingFuture<Vec<Resource>>
    >
}

impl<'a, TE,D,C> Future for RenderPreparationFuture<'a, TE, D, C>
    where TE: TemplateEngine, TE: UseTemplateEngine<D>, C: Context
{
    type Item = Preparations<'a, TE, D, C>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let (
            inline_embeddings,
            attachments
        ) = try_ready!(self.loading_fut.poll());

        //UNWRAP_SAFE only non if polled after resolved
        let (template, prepared_data, ctx) = self.payload.take().unwrap();

        Ok(Async::Ready(Preparations {
            template,
            prepared_data,
            ctx,
            inline_embeddings,
            attachments
        }))
    }
}

pub struct Preparations<'a, TE, D, C>
    where TE: TemplateEngine + UseTemplateEngine<D>, C: Context
{
    template: Template<TE>,
    prepared_data: Bound<'a, <TE as UseTemplateEngine<D>>::PreparedData>,
    ctx: C,
    inline_embeddings: HashMap<String, Resource>,
    attachments: Vec<Resource>
}

impl<'a, TE, D, C> Preparations<'a, TE, D, C>
    where TE: TemplateEngine, TE: UseTemplateEngine<D>, C: Context
{
    pub fn render_to_mail_parts(self) -> Result<(MailParts, Header<headers::Subject>), Error> {
        let Preparations {
            template,
            prepared_data,
            ctx,
            inline_embeddings,
            mut attachments
        } = self;

        let subject = template.engine().render(
            template.subject_template_id(),
            &prepared_data,
            AdditionalCIds::new(&[])
        )?;

        let subject = headers::Subject::auto_body(subject)?;

        //TODO use Vec1 try_map instead of loop
        let mut bodies = Vec::new();
        for body in template.bodies().iter() {
            let raw = template.engine().render(
                body.template_id(),
                &prepared_data,
                AdditionalCIds::new(&[
                    &inline_embeddings,
                    body.inline_embeddings(),
                    template.inline_embeddings()
                ])
            )?;

            let data = Data::new(
                raw.into_bytes(),
                Metadata {
                    file_meta: Default::default(),
                    media_type: body.media_type().clone(),
                    content_id: ctx.generate_content_id()
                }
            );

            let inline_embeddings = body.inline_embeddings()
                .values()
                .cloned()
                .collect();

            bodies.push(BodyPart {
                resource: Resource::Data(data),
                inline_embeddings,
                attachments: Vec::new()
            });
        }

        attachments.extend(template.attachments().iter().cloned());

        let mut inline_embeddings_vec = Vec::new();
        for (key, val) in template.inline_embeddings() {
            if !inline_embeddings.contains_key(key) {
                inline_embeddings_vec.push(val.clone())
            }
        }

        inline_embeddings_vec.extend(inline_embeddings.into_iter().map(|(_,v)|v));

        let parts = MailParts {
            //UNWRAP_SAFE (complexly mapping a Vec1 is safe)
            alternative_bodies: Vec1::from_vec(bodies).unwrap(),
            inline_embeddings: inline_embeddings_vec,
            attachments
        };

        Ok((parts, subject))
    }

    pub fn render(self) -> Result<Mail, Error> {
        let (parts, subject) = self.render_to_mail_parts()?;
        let mut mail = parts.compose();
        mail.insert_header(subject);
        Ok(mail)
    }
}

#[derive(Debug)]
pub struct BodyTemplate<TE: TemplateEngine> {
    template_id: TE::Id,
    media_type: MediaType,
    embeddings: HashMap<String, Resource>
    //TODO potential additional fields like file_name maybe attachments
}

impl<TE> BodyTemplate<TE>
    where TE: TemplateEngine
{
    pub fn template_id(&self) -> &TE::Id {
        &self.template_id
    }

    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    pub fn inline_embeddings(&self) -> &HashMap<String, Resource> {
        &self.embeddings
    }
}

#[derive(Debug)]
pub struct Subject<TE: TemplateEngine> {
    template_id: TE::Id
}

impl<TE> Subject<TE>
    where TE: TemplateEngine
{
    pub fn template_id(&self) -> &TE::Id {
        &self.template_id
    }
}
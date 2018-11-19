use std::{mem, fs};

use galemu::{Bound, BoundExt};
use serde::Deserialize;
use failure::Fail;
use futures::{
    Future, Poll, Async,
    try_ready,
    future::{
        self,
        JoinAll, Either, FutureResult
    }
};
use mail_base::Source;

mod serde_impl;
mod base_dir;
mod path_rebase;

pub use self::base_dir::CwdBaseDir;
pub use self::path_rebase::PathRebaseable;

pub trait TemplateEngine {
    type Id: Debug;
    type Error: Fail;

    type LazyBodyTemplate: PathRebaseable + Debug + for<'a> Deserialize<'a>;

    fn load_body_template(&mut self, tmpl: Self::LazyBodyTemplate)
        -> Result<BodyTemplate<Self>, TODO>;

    fn load_subject_template(&mut self, template_string: String)
        -> Result<Self::Id, TODO>;

    pub fn load_template_from_path<P>(self, path: P) -> Result<Self, TODO>
        where P: AsRef<Path>
    {
        let content = fs::read_to_string(path)?;
        //TODO choose serde serializer by file extension (toml, json)
        // then serialize to TemplateBase
        // then `base.with_engine(self)`
        load_template_from_str(&content)
    }

    pub fn load_template_from_str(self, desc: &str) -> Result<Self, TODO> {
        self::load_template::from_str(desc)
    }
}

pub struct PreparationData<'a, D: for<'a> BoundExt<'a>> {
    pub attachments: Vec<Resource>,
    pub inline_embeddings: HashMap<String, Resource>,
    pub prepared_data: Bound<'a, D>
}

pub trait UseTemplateEngine<D>: TemplateEngine {

    //TODO[doc]: this is needed for all template engines which use to json serialization
    // (we have more then one template so there would be a lot of overhead)
    type PreparedData: for<'a> BoundExt<'a>;

    //TODO[design]: allow returning a result
    fn prepare_data<'a>(raw: &'a D) -> PreparationData<'a, Self::PreparedData>;

    fn render(
        &self,
        id: &Self::Id,
        data: &Bound<'a, Self::PreparedData>,
        additional_cids: AdditionalCids
    ) -> Result<String, Self::Error>;
}

#[derive(Debug)]
pub struct Template<TE: TemplateEngine> {
    inner: Arc<InnerTemplate<TE>>
}

struct InnerTemplate<TE: TemplateEngine> {
    template_name: String,
    base_dir: CwdBaseDir,
    subject: Subject,
    /// This can only be in the loaded form _iff_ this is coupled
    /// with a template engine instance, as using it with the wrong
    /// template engine will lead to potential bugs and panics.
    bodies: Vec1<BodyTemplate<TE>>,
    //TODO: make sure
    embeddings: HashMap<String, Resource>,
    attachments: Vec<Resource>,
    engine: TE,
}

type Embeddings = HashMap<String, Resource>;
type Attachments = Vec<Resource>;


pub trait TemplateExt<D, TE> {
    fn prepare_to_render<C>(&self, data: &D, ctx: &C) -> RenderPreparationFuture<TE, D, C>;
}


impl<D, TE> TemplateExt<D, TE> for Template<TE>
    where TE: UseTemplateEngine<D>
{
    fn prepare_to_render<: Context>(&self, data: &D, ctx: &C) ->
        MailPreparationFuture<D, TE, C>
    {
        let preps = self.engine.prepare_data(data);

        let PreparationData {
            inline_embeddings,
            attachments,
            prepare_data
        } = self;

        let loading_fut = Resource::load_container(inline_embeddings, ctx)
            .join(Resource::load_container(attachments, ctx));

        RenderPreparationFuture {
            template: self.clone(),
            context: ctx.clone(),
            prepare_data,
            loading_fut
        }
    }
}

pub struct RenderPreparationFuture<TE, D, C> {
    payload: Option<(
        Template<TE>,
        <TE as UseTemplateEngine<D>>::PreparationData,
        C
    )>,
    loading_fut: Join<
        ResourceContainerLoadingFuture<HashMap<String, Resource>>,
        ResourceContainerLoadingFuture<Vec<Resource>>
    >
}

impl<TE,D,C> Future for RenderPreparationFuture<TE, D, C>
    TE: TemplateEngine, TE: UseTemplateEngine<D>, C: Context
{
    type Item = Preparations<TE, D, C>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let (
            inline_embeddings,
            attachments
        ) = try_ready!(&mut self.loading_fut);

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

pub struct Preparations<TE, D, C> {
    template: Template<TE>,
    prepared_data: <TE as UseTemplateEngine<D>>::PreparationData,
    ctx: C,
    inline_embeddings: HashMap<String, Resource>,
    attachemnts: Vec<Resource>
}

impl<TE, D, C> Preparations<TE, D, C>
    where TE: TemplateEngine, TE: UseTemplateEngine<D>, C: Context
{
    pub fn render_to_mail_parts(self) -> Result<MailParts, Error> {
        let Preparations {
            template,
            prepared_data,
            ctx,
            //UPS thats a hash map not a Vec
            inline_embeddings: inline_embeddings_from_data,
            attachemnts
        } = self;

        let subject = template.engine().render(
            template.subject_template_id(),
            &prepare_data,
            AdditionalCids::new(&[])
        )?;

        //TODO use Vec1 try_map instead of loop
        let mut bodies = Vec::new();
        for body in template.bodies().iter() {
            let raw = self.engine.render(
                body.template_id(),
                &prepare_data,
                AdditionalCids::new(&[
                    &inline_embeddings
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

            let inline_embeddings = body.embeddings()
                .values()
                .cloned()
                .collect();

            bodies.push(BodyPart {
                resource: Resource::Data(data)
                inline_embeddings
            });
        }

        Ok(MailParts {
            //UNWRAP_SAFE (complexly mapping a Vec1 is safe)
            alternative_bodies: Vec1::new(bodies).unwrap(),
            inline_embeddings: template.embeddings().values().cloned().collect(),
            attachments: template.attachments().clone()
        })
    }

    pub fn render(self) -> Result<Mail, Error> {
        let parts = self.render_to_mail_parts()?;
        //PANIC_SAFE: templates load all data to at last the point where it has a content id.
        let mail = parts.compose_without_generating_content_ids()?;
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

    pub fn embeddings(&self) -> &HashMap<String, Resource> {
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

//--------------------

pub struct AdditionalCids<'a> {
    additional: &'a [&'a HashMap<String, Resource>]
}




// pub struct AdditionalCIds<'a> {
//     additional_resources: &'a [&'a HashMap<String, EmbeddedWithCId>]
// }

// impl<'a> AdditionalCIds<'a> {

//     pub fn new(additional_resources: &'a [&'a HashMap<String, EmbeddedWithCId>]) -> Self {
//         AdditionalCIds { additional_resources }
//     }


//     /// returns the content id associated with the given name
//     ///
//     /// If multiple of the maps used to create this type contain the
//     /// key the first match is returned and all later ones are ignored.
//     pub fn get(&self, name: &str) -> Option<&ContentId> {
//         for possible_source in self.additional_resources {
//             if let Some(res) = possible_source.get(name) {
//                 return Some(res.content_id());
//             }
//         }
//         return None;
//     }
// }

// impl<'a> Serialize for AdditionalCIds<'a> {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//         where S: Serializer
//     {
//         let mut existing_keys = HashSet::new();
//         serializer.collect_map(
//             self.additional_resources
//             .iter()
//             .flat_map(|m| m.iter().map(|(k, v)| (k, v.content_id())))
//             .filter(|key| existing_keys.insert(key.to_owned()))
//         )
//     }
// }


use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::mem::replace;

use serde::{Serialize, Serializer};

use ::template_engine_prelude::*;
use mail::file_buffer::FileBuffer;
use mail::MediaType;

use self::error::{SpecError, Error, Result};
use self::utils::{new_string_path, string_path_set, check_string_path};

pub mod error;
mod utils;
mod settings;
pub use self::settings::*;
mod from_dir;

pub trait RenderEngine {
    type Error: StdError + Send + 'static;

    //any caching is done inside transparently
    fn render<D: Serialize>(&self, id: &str, data: D) -> StdResult<String, Self::Error>;

}

#[derive(Debug)]
pub struct RenderTemplateEngine<R: RenderEngine> {
    render_engine: R,
    id2spec: HashMap<String, TemplateSpec>,
}

impl<R> RenderTemplateEngine<R>
    where R: RenderEngine
{

    pub fn lookup_spec(&self, template_id: &str) -> Result<&TemplateSpec, R::Error> {
        self.id2spec
            .get(template_id)
            .ok_or_else(|| Error::UnknownTemplateId(template_id.to_owned()))
    }

}

impl<R, C> TemplateEngine<C> for RenderTemplateEngine<R>
    where R: RenderEngine, C: Context
{
    type TemplateId = str;
    type Error = Error<R::Error>;

    fn templates<D: Serialize>(
        &self,
        ctx: &C,
        template_id: &str,
        data: &D
    ) -> StdResult<MailParts, Self::Error >
    {
        let spec = self.lookup_spec(template_id)?;

        //OPTIMIZE there should be a more efficient way
        // maybe use Rc<str> as keys? and Rc<ResourceSpec> for embeddings?
        let shared_embeddings = spec.embeddings().iter()
            .map(|(key, resource_spec)|
                create_embedding(key.to_owned(),resource_spec.clone(), ctx))
            .collect::<Result<HashMap<_,_>,_>>()?;

        let mut attachments = Vec::new();
        let bodies = spec.sub_specs().try_mapped_ref(|template| {

            let embeddings = template.embeddings.iter()
                .map(|(key, resource_spec)|
                    create_embedding(key.to_owned(),resource_spec.clone(), ctx))
                .collect::<Result<HashMap<_,_>,_>>()?;


            //TODO fix newlines in rendered
            let rendered = {
                // make CIds available to render engine
                let data = DataWrapper { data, cids: (&embeddings, &shared_embeddings) };
                let path = template.str_path();
                self.render_engine.render(&*path, data)
                    .map_err(|re| Error::RenderError(re))?
            };

            let buffer = FileBuffer::new(template.media_type().clone(), rendered.into());
            let resource = Resource::from_buffer(buffer);

            attachments.extend(template.attachments().iter()
                .map(|resouce_spec| {
                    let resource = Resource::from_spec(resouce_spec.clone());
                    Attachment::new(resource)
                }));

            Ok(BodyPart {
                body_resource: resource,
                embeddings: embeddings.into_iter().map(|(_,v)| v).collect()
            })
        })?;

        Ok(MailParts {
            alternative_bodies: bodies,
            shared_embeddings: shared_embeddings.into_iter().map(|(_, v)| v).collect(),
            attachments,
        })
    }
}

fn create_embedding<R, C>(key: String, resource_spec: ResourceSpec, ctx: &C)
    -> Result<(String, EmbeddingWithCId), R>
    where C: Context, R: StdError
{
    let resource = Resource::from_spec(resource_spec.clone());
    let cid = ctx.new_content_id().map_err(|err| Error::CIdGenFailed(err))?;
    Ok((key, EmbeddingWithCId::new(resource, cid)))
}


#[derive(Debug)]
pub struct TemplateSpec {
    /// the `base_path` which was used to construct the template from,
    /// e.g. with `TemplateSpec::from_dir` and which is used for reloading
    base_path: Option<PathBuf>,
    templates: Vec1<SubTemplateSpec>,
    /// template level embeddings, i.e. embeddings shared between alternative bodies
    embeddings: HashMap<String, ResourceSpec>
}

impl TemplateSpec {

    ///
    /// ```no_rust
    /// templates/
    ///  templateA/
    ///   html/
    ///     mail.html
    ///     emb_logo.png
    ///   text/
    ///     mail.text
    /// ```
    ///
    /// Note:  the file name "this.is.a" is interprete as name "this" with suffix/type ".is.a"
    ///        so it's cid gan be accessed with "cids.this"
    #[inline]
    pub fn from_dir<P>(base_path: P, settings: &Settings) -> StdResult<TemplateSpec, SpecError>
        where P: AsRef<Path>
    {
        from_dir::from_dir(base_path.as_ref(), settings)
    }

    pub fn new(templates: Vec1<SubTemplateSpec>) -> Self {
        Self::new_with_embeddings(templates, Default::default())
    }

    pub fn new_with_embeddings(
        templates: Vec1<SubTemplateSpec>,
        embeddings: HashMap<String, ResourceSpec>
    ) -> Self {
        TemplateSpec { base_path: None, templates, embeddings }
    }

    pub fn new_with_base_path<P>(templates: Vec1<SubTemplateSpec>, base_path: P)
        -> StdResult<Self, SpecError>
        where P: AsRef<Path>
    {
        Self::new_with_embeddings_and_base_path(
            templates, Default::default(), base_path.as_ref()
        )
    }

    pub fn new_with_embeddings_and_base_path<P>(
        templates: Vec1<SubTemplateSpec>,
        embeddings: HashMap<String, ResourceSpec>,
        base_path: P
    ) -> StdResult<Self, SpecError>
        where P: AsRef<Path>
    {
        let path = base_path.as_ref().to_owned();
        check_string_path(&*path)?;
        Ok(TemplateSpec { base_path: Some(path), templates, embeddings })
    }

    pub fn sub_specs(&self) -> &Vec1<SubTemplateSpec> {
        &self.templates
    }

    pub fn sub_specs_mut(&mut self) -> &mut Vec1<SubTemplateSpec> {
        &mut self.templates
    }

    pub fn embeddings(&self) -> &HashMap<String, ResourceSpec> {
        &self.embeddings
    }

    pub fn embeddings_mut(&mut self) -> &mut HashMap<String, ResourceSpec> {
        &mut self.embeddings
    }


    pub fn base_path(&self) -> Option<&Path> {
        self.base_path.as_ref().map(|r| &**r)
    }

    pub fn set_base_path<P>(&mut self, new_path: P) -> StdResult<Option<PathBuf>, SpecError>
        where P: AsRef<Path>
    {
        let path = new_path.as_ref();
        check_string_path(path)?;
        Ok(replace(&mut self.base_path, Some(path.to_owned())))
    }

}

#[derive(Debug)]
pub struct SubTemplateSpec {
    media_type: MediaType,
    /// The path to the template file if it is a relative path it is
    /// used relative to the working directory
    path: String,
    // (Name, ResourceSpec) | name is used by the template engine e.g. log, and differs to
    // resource spec use_name which would
    //  e.g. be logo.png but referring to the file long_logo_name.png
    embeddings: HashMap<String, ResourceSpec>,//todo use ordered map
    attachments: Vec<ResourceSpec>
}

impl SubTemplateSpec {

    //FIXME to many arguments alternatives: builder,
    // default values (embedding, attachment)+then setter,
    // default values + then with_... methods
    pub fn new<P>(path: P,
                  media_type: MediaType,
                  embeddings: HashMap<String, ResourceSpec>,
                  attachments: Vec<ResourceSpec>
    ) -> StdResult<Self, SpecError>
        where P: AsRef<Path>
    {
        let path = new_string_path(path.as_ref())?;
        Ok(SubTemplateSpec { path, media_type, embeddings, attachments })
    }

    pub fn path(&self) -> &Path {
        Path::new(&self.path)
    }

    pub fn str_path(&self) -> &str {
        &self.path
    }

    pub fn set_path<P>(&mut self, new_path: P) -> StdResult<PathBuf, SpecError>
        where P: AsRef<Path>
    {
        string_path_set(&mut self.path, new_path.as_ref())
    }

    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    pub fn set_media_type(&mut self, media_type: MediaType) -> MediaType {
        //we might wan't to add restrictions at some point,e.g. no multipart mediatype
        replace(&mut self.media_type, media_type)
    }

    pub fn embeddings(&self) -> &HashMap<String, ResourceSpec> {
        &self.embeddings
    }

    pub fn embedding_mut(&mut self) -> &mut HashMap<String, ResourceSpec> {
        &mut self.embeddings
    }

    pub fn attachments(&self) -> &Vec<ResourceSpec> {
        &self.attachments
    }

    pub fn attachments_mut(&mut self) -> &mut Vec<ResourceSpec> {
        &mut self.attachments
    }

}


#[derive(Debug, Serialize)]
struct DataWrapper<'a, D: Serialize + 'a> {
    /// make cid's of embeddings available
    #[serde(serialize_with = "cid_mapped_serialize")]
    pub cids: (&'a HashMap<String, EmbeddingWithCId>, &'a HashMap<String, EmbeddingWithCId>),
    /// make data available
    pub data: &'a D
}

/// serialize name->embedding_cid map as name->cid map
fn cid_mapped_serialize<'a, S>(
    cids: &(&'a HashMap<String, EmbeddingWithCId>, &'a HashMap<String, EmbeddingWithCId>),
    serializer: S
) -> StdResult<S::Ok, S::Error>
    where S: Serializer
{
    serializer.collect_map(cids.0.iter().chain(cids.1.iter()).map(|(k, v)| {
        (k, v.content_id().as_str())
    }))
}


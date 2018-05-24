use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::mem::replace;

use failure::Fail;
use serde::{Serialize, Serializer};
use vec1::Vec1;

use mail::{Resource, Context};
use mail::file_buffer::FileBuffer;
use headers::components::MediaType;

use ::TemplateEngine;
use ::template::{BodyPart, MailParts};
use ::resource::{Attachment, EmbeddingWithCId};


use self::error::{Error, LoadingSpecError, LoadingSpecErrorVariant};
use self::utils::{new_string_path, string_path_set, check_string_path, fix_newlines};

pub mod error;
mod utils;
mod settings;
pub use self::settings::*;
mod from_dir;

pub trait RenderEngine {
    const PRODUCES_VALID_NEWLINES: bool;
    type Error: Fail;
    //any caching is done inside transparently
    fn render<D: Serialize>(&self, id: &str, data: &D) -> Result<String, Self::Error>;

}

#[derive(Debug)]
pub struct RenderTemplateEngine<R: RenderEngine> {
    fix_newlines: bool,
    render_engine: R,
    id2spec: HashMap<String, TemplateSpec>,
}

impl<R> RenderTemplateEngine<R>
    where R: RenderEngine
{
    pub fn new(render_engine: R) -> Self {
        RenderTemplateEngine {
            render_engine,
            id2spec: Default::default(),
            fix_newlines: !R::PRODUCES_VALID_NEWLINES,
        }
    }

    pub fn set_fix_newlines(&mut self, should_fix_newlines: bool) {
        self.fix_newlines = should_fix_newlines
    }

    pub fn does_fix_newlines(&self) -> bool {
        self.fix_newlines
    }

    pub fn add_spec(&mut self, id: String, spec: TemplateSpec) -> Option<TemplateSpec> {
        self.id2spec.insert(id, spec)
    }

    pub fn remove_spec(&mut self, id: &str) -> Option<TemplateSpec> {
        self.id2spec.remove(id)
    }

    pub fn specs(&self) -> &HashMap<String, TemplateSpec> {
        &self.id2spec
    }

//    pub fn specs_mut(&mut self) -> &mut HashMap<String, TemplateSpec> {
//        &mut self.specs()
//    }

    pub fn lookup_spec(&self, template_id: &str) -> Result<&TemplateSpec, Error<R::Error>> {
        self.id2spec
            .get(template_id)
            .ok_or_else(|| Error::from_unknown_template_id(template_id))
    }

    pub fn load_specs_from_dir<P>(
        &mut self,
        dir_path: P,
        settings: &LoadSpecSettings
    ) -> Result<(), LoadingSpecError>
        where P: AsRef<Path>
    {
        self._load_specs_from_dir(dir_path.as_ref(), settings, false)
    }

    pub fn load_specs_from_dir_allow_override<P>(
        &mut self,
        dir_path: P,
        settings: &LoadSpecSettings
    ) -> Result<(), LoadingSpecError>
        where P: AsRef<Path>
    {
        self._load_specs_from_dir(dir_path.as_ref(), settings, true)
    }

    fn _load_specs_from_dir(
        &mut self,
        dir_path: &Path,
        settings: &LoadSpecSettings,
        allow_override: bool
    ) -> Result<(), LoadingSpecError>
    {
        for entry in dir_path.read_dir()? {
            let entry = entry?;
            if entry.metadata()?.is_dir() {
                let id = entry.file_name()
                    .into_string()
                    .map_err(|file_name| LoadingSpecErrorVariant::NonStringPath(file_name.into()))?;
                let spec = TemplateSpec::from_dir(entry.path(), settings)?;
                let old = self.add_spec(id, spec);
                if old.is_some() && !allow_override {
                    // we already know that the file name can be converted into a string
                    let file_name = entry.file_name().into_string().unwrap();
                    return Err(LoadingSpecErrorVariant::AccidentalSpecOverride { id: file_name }.into());
                }
            }
        }
        Ok(())
    }

}

impl<R, C> TemplateEngine<C> for RenderTemplateEngine<R>
    where R: RenderEngine, C: Context
{
    type TemplateId = str;
    type Error = Error<R::Error>;

    fn use_templates<D: Serialize>(
        &self,
        ctx: &C,
        template_id: &str,
        data: &D
    ) -> Result<MailParts, Self::Error >
    {
        let spec = self.lookup_spec(template_id)?;

        //OPTIMIZE there should be a more efficient way
        // maybe use Rc<str> as keys? and Rc<Resource> for embeddings?
        let shared_embeddings = spec.embeddings().iter()
            .map(|(key, resource_spec)|
                create_embedding(key.to_owned(),resource_spec.clone(), ctx))
            .collect::<HashMap<_,_>>();

        let mut attachments = Vec::new();
        let bodies = spec.sub_specs().try_mapped_ref(|template| {

            let embeddings = template.embeddings.iter()
                .map(|(key, resource_spec)|
                    create_embedding(key.to_owned(),resource_spec.clone(), ctx))
                .collect::<HashMap<_,_>>();

            let rendered = {
                // make CIds available to render engine
                let data = DataWrapper { data, cids: (&embeddings, &shared_embeddings) };
                let path = template.str_path();
                self.render_engine.render(&*path, &data)
                    .map_err(|re| Error::RenderError(re))?
            };

            let rendered =
                if self.fix_newlines {
                    fix_newlines(rendered)
                } else {
                    rendered
                };

            let buffer = FileBuffer::new(template.media_type().clone(), rendered.into());
            let resource = Resource::sourceless_from_buffer(buffer);

            attachments.extend(template.attachments().iter()
                .map(|resource| Attachment::new(resource.clone())));

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

fn create_embedding<C>(key: String, resource: Resource, ctx: &C) -> (String, EmbeddingWithCId)
    where C: Context
{
    let cid = ctx.generate_content_id();
    (key, EmbeddingWithCId::new(resource, cid))
}


#[derive(Debug)]
pub struct TemplateSpec {
    /// the `base_path` which was used to construct the template from,
    /// e.g. with `TemplateSpec::from_dir` and which is used for reloading
    base_path: Option<PathBuf>,
    templates: Vec1<SubTemplateSpec>,
    /// template level embeddings, i.e. embeddings shared between alternative bodies
    embeddings: HashMap<String, Resource>
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
    pub fn from_dir<P>(base_path: P, settings: &LoadSpecSettings) -> Result<TemplateSpec, LoadingSpecError>
        where P: AsRef<Path>
    {
        from_dir::from_dir(base_path.as_ref(), settings)
    }

    pub fn new(templates: Vec1<SubTemplateSpec>) -> Self {
        Self::new_with_embeddings(templates, Default::default())
    }

    pub fn new_with_embeddings(
        templates: Vec1<SubTemplateSpec>,
        embeddings: HashMap<String, Resource>
    ) -> Self {
        TemplateSpec { base_path: None, templates, embeddings }
    }

    pub fn new_with_base_path<P>(templates: Vec1<SubTemplateSpec>, base_path: P)
        -> Result<Self, LoadingSpecError>
        where P: AsRef<Path>
    {
        Self::new_with_embeddings_and_base_path(
            templates, Default::default(), base_path.as_ref()
        )
    }

    pub fn new_with_embeddings_and_base_path<P>(
        templates: Vec1<SubTemplateSpec>,
        embeddings: HashMap<String, Resource>,
        base_path: P
    ) -> Result<Self, LoadingSpecError>
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

    pub fn embeddings(&self) -> &HashMap<String, Resource> {
        &self.embeddings
    }

    pub fn embeddings_mut(&mut self) -> &mut HashMap<String, Resource> {
        &mut self.embeddings
    }


    pub fn base_path(&self) -> Option<&Path> {
        self.base_path.as_ref().map(|r| &**r)
    }

    pub fn set_base_path<P>(&mut self, new_path: P) -> Result<Option<PathBuf>, LoadingSpecError>
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
    // (Name, Resource) | name is used by the template engine e.g. log, and differs to
    // resource spec use_name which would
    //  e.g. be logo.png but referring to the file long_logo_name.png
    embeddings: HashMap<String, Resource>,//todo use ordered map
    attachments: Vec<Resource>
}

impl SubTemplateSpec {

    //FIXME to many arguments alternatives: builder,
    // default values (embedding, attachment)+then setter,
    // default values + then with_... methods
    pub fn new<P>(path: P,
                  media_type: MediaType,
                  embeddings: HashMap<String, Resource>,
                  attachments: Vec<Resource>
    ) -> Result<Self, LoadingSpecError>
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

    pub fn set_path<P>(&mut self, new_path: P) -> Result<PathBuf, LoadingSpecError>
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

    pub fn embeddings(&self) -> &HashMap<String, Resource> {
        &self.embeddings
    }

    pub fn embedding_mut(&mut self) -> &mut HashMap<String, Resource> {
        &mut self.embeddings
    }

    pub fn attachments(&self) -> &Vec<Resource> {
        &self.attachments
    }

    pub fn attachments_mut(&mut self) -> &mut Vec<Resource> {
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
) -> Result<S::Ok, S::Error>
    where S: Serializer
{
    serializer.collect_map(cids.0.iter().chain(cids.1.iter()).map(|(k, v)| {
        (k, v.content_id().as_str())
    }))
}


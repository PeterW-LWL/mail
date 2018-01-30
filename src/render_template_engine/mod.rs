use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::borrow::Cow;
use serde::{Serialize, Serializer};

use ::template_engine_prelude::*;
use mail::file_buffer::FileBuffer;
use mail::MediaType;

use self::error::{Error, Result};


mod error;

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
    ) -> StdResult<(Vec1<TemplateBody>, Vec<Attachment>), Self::Error >
    {
        let spec = self.lookup_spec(template_id)?;
        let mut attachments = Vec::new();
        let templates = spec.templates().try_mapped_ref(|template| {

            let embeddings = template.embeddings.iter()
                .map(|(key, resource_spec)| {
                    let resource = Resource::from_spec(resource_spec.clone());
                    let cid = ctx.new_content_id().map_err(|err| Error::CIdGenFailed(err))?;
                    let embedding = EmbeddingWithCId::new(resource, cid);
                    Ok((key.to_owned(), embedding))
                })
                .collect::<Result<HashMap<_,_>,_>>()?;


            //TODO fix newlines in rendered
            let rendered = {
                // make CIds available to render engine
                let data = DataWrapper { data, cids: &embeddings };
                let path = template.path(spec.base_path());
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

            Ok(TemplateBody {
                body_resource: resource,
                embeddings
            })
        })?;
        Ok((templates, attachments))

    }
}


pub trait RenderEngine {
    type Error: StdError + Send + 'static;

    //any chaching is doen inside transparently
    fn render<D: Serialize>(&self, id: &str, data: D) -> StdResult<String, Self::Error>;

}

/// POD
#[derive(Debug)]
pub struct TemplateSpec {
    /// the `base_path` to which `SubTemplateSpec` paths can be relative to
    ///
    /// Note that it is used as part of a render_id for RenderEngine and therefore
    /// has to be a valid utf-8 string, instead of Path.
    pub base_path: String,
    pub templates: Vec1<SubTemplateSpec>
}

impl TemplateSpec {
//    pub fn from_dir(path: Path) -> TemplateSpec {
//        //create template spec defaulting to:
//        // 1. order is html, xhtml, txt (or the other name around?)
//        // 2. expect a folder for each sub_template (html,txt,etc.)
//        // 3. in that folder expect a file "mail.<sub_template>" e.g. mail.html
//        // 4. all other resources in the folder are expected to be embeddings with
//        // 4.1. media_type = auto i.e. sniff it (sane sniffer?)
//        // 4.2. name = strip_suffix(file_name(path_to_file))
//        // 5. charset is always utf-8
//        // 6. no attachments
//    }

    pub fn templates(&self) -> &Vec1<SubTemplateSpec> {
        &self.templates
    }

    pub fn base_path(&self) -> &str {
        &self.base_path
    }
}

#[derive(Debug)]
pub struct SubTemplateSpec {
    media_type: MediaType,
    /// The path to the template file
    ///
    /// It can either be a path relative to the `TemplateSpec::base_path()` or
    /// an absolute path.
    ///
    /// Note that it is a string, as it is also used as a Id for the template engine,
    /// and not all engine support Path based Id's, so the
    path: String,
    // (Name, ResourceSpec) | name is used by the template engine e.g. log, and differs to
    // resource spec use_name which would
    //  e.g. be logo.png but referring to the file long_logo_name.png
    embeddings: HashMap<String, ResourceSpec>,//todo use ordered map
    attachments: Vec<ResourceSpec>
}

impl SubTemplateSpec {

    pub fn path(&self, base_path: &str) -> Cow<str> {
        if Path::new(&*self.path).is_absolute() {
            Cow::Borrowed(&*self.path)
        } else {
            let full_path: PathBuf = Path::new(base_path).join(&self.path);
            //UNWRAP_SAFE: we create the Path by joinging to strings
            Cow::Owned(full_path.into_os_string().into_string().unwrap())
        }
    }

    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    pub fn embeddings(&self) -> &HashMap<String, ResourceSpec> {
        &self.embeddings
    }

    pub fn attachments(&self) -> &[ResourceSpec] {
        &self.attachments
    }
}


#[derive(Debug, Serialize)]
struct DataWrapper<'a, D: Serialize + 'a> {
    /// make cid's of embeddings available
    #[serde(serialize_with = "cid_mapped_serialize")]
    pub cids: &'a HashMap<String, EmbeddingWithCId>,
    /// make data available
    pub data: &'a D
}

/// serialize name->embedding_cid map as name->cid map
fn cid_mapped_serialize<'a, S>(
    cids: &&'a HashMap<String, EmbeddingWithCId>,
    serializer: S
) -> StdResult<S::Ok, S::Error>
    where S: Serializer
{
    serializer.collect_map(cids.iter().map(|(k, v)| {
        (k, v.content_id().as_str())
    }))
}

use std::{
    collections::HashMap,
    sync::Arc
};

use serde::{
    Serialize, Deserialize,
    de::{
        Deserializer,
    },
};
use failure::Error;
use vec1::Vec1;

use mail_core::{Resource, Source, IRI};

use super::{
    Template,
    TemplateEngine,
    CwdBaseDir,
    PathRebaseable,
    InnerTemplate,
    Subject,
};

/// Type used when deserializing a template using serde.
///
/// This type should only be used as intermediate type
/// used for deserialization as templates need to be
/// bundled with a template engine.
///
/// # Serialize/Deserialize
///
/// The derserialization currently only works with
/// self-describing data formats.
///
/// There are a number of shortcuts to deserialize
/// resources (from emebddings and/or attachments):
///
/// - Resources can be deserialized normally (from a externally tagged enum)
/// - Resources can be deserialized from the serialized repr of `Source`
/// - Resources can be deserialized from a string which is used to create
///   a `Resource::Source` with a iri using the `path` scheme and the string
///   content as the iris "tail".
#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateBase<TE: TemplateEngine> {
    #[serde(rename="name")]
    template_name: String,
    base_dir: CwdBaseDir,
    subject: LazySubject,
    bodies: Vec1<TE::LazyBodyTemplate>,
    //TODO impl. deserialize where
    // resource:String -> IRI::new("path", resource) -> Resource::Source
    #[serde(deserialize_with="deserialize_embeddings")]
    embeddings: HashMap<String, Resource>,
    #[serde(deserialize_with="deserialize_attachments")]
    attachments: Vec<Resource>,
}

impl<TE> TemplateBase<TE>
    where TE: TemplateEngine
{

    //TODO!! make this load all embeddings/attachments and make it a future
    /// Couples the template base with a specific engine instance.
    pub fn load(self, mut engine: TE) -> Result<Template<TE>, Error> {
        let TemplateBase {
            template_name,
            base_dir,
            subject,
            bodies,
            mut embeddings,
            mut attachments
        } = self;

        let subject = Subject{ template_id: engine.load_subject_template(subject.template_string)? };

        let bodies = bodies.try_mapped(|mut lazy_body| -> Result<_, Error> {
            lazy_body.rebase_to_include_base_dir(&base_dir)?;
            Ok(engine.load_body_template(lazy_body)?)
        })?;


        for embedding in embeddings.values_mut() {
            embedding.rebase_to_include_base_dir(&base_dir)?;
        }

        for attachment in attachments.iter_mut() {
            attachment.rebase_to_include_base_dir(&base_dir)?;
        }


        let inner = InnerTemplate {
            template_name,
            base_dir,
            subject,
            bodies,
            embeddings,
            attachments,
            engine
        };

        Ok(Template { inner: Arc::new(inner) })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct LazySubject {
    #[serde(flatten)]
    template_string: String
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ResourceDeserializationHelper {
    // Note: VARIANT ORDER MATTERS (serde,untagged)
    // This allows specifying resources in three ways.
    // 1. as tagged enum `Resource` (e.g. `{"Source": { "iri": ...}}}`)
    // 2. as struct `Source` (e.g. `{"iri": ...}` )
    // 3. as String which is interpreted as path iri
    Normal(Resource),
    FromSource(Source),
    FromString(String)
}

impl Into<Resource> for ResourceDeserializationHelper {
    fn into(self) -> Resource {
        use self::ResourceDeserializationHelper::*;
        match self {
            Normal(resource) => resource,
            FromString(string) => {
                let source = Source {
                    //UNWRAP_SAFE: only scheme validation could fail,
                    // but its static "path" which is known to be valid
                    iri: IRI::from_parts("path", &string).unwrap(),
                    use_media_type: Default::default(),
                    use_file_name: Default::default()
                };

                Resource::Source(source)
            },
            FromSource(source) => Resource::Source(source)
        }
    }
}

fn deserialize_embeddings<'de, D>(deserializer: D)
    -> Result<HashMap<String, Resource>, D::Error>
    where D: Deserializer<'de>
{
    //FIXME[perf] write custom visitor etc.
    let map = <HashMap<String, ResourceDeserializationHelper>>
        ::deserialize(deserializer)?;

    let map = map.into_iter()
        .map(|(k, helper)| (k, helper.into()))
        .collect();

    Ok(map)
}

fn deserialize_attachments<'de, D>(deserializer: D)
    -> Result<Vec<Resource>, D::Error>
    where D: Deserializer<'de>
{
    //FIXME[perf] write custom visitor etc.
    let vec = <Vec<ResourceDeserializationHelper>>
        ::deserialize(deserializer)?;

    let vec = vec.into_iter()
        .map(|helper| helper.into())
        .collect();

    Ok(vec)
}
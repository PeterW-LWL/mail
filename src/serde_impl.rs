use std::{
    collections::HashMap,
    sync::Arc,
    path::Path
};

use serde::{
    Serialize, Deserialize,
    de::{
        Deserializer,
    },
};
use failure::Error;
use futures::{Future, future::{self, Either}};
use vec1::Vec1;

use mail_core::{Resource, Source, IRI, Context};

use super::{
    Template,
    TemplateEngine,
    CwdBaseDir,
    PathRebaseable,
    InnerTemplate,
    Subject,
    UnsupportedPathError
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
    pub fn load(self, mut engine: TE, ctx: &impl Context) -> impl Future<Item=Template<TE>, Error=Error> {
        let TemplateBase {
            template_name,
            base_dir,
            subject,
            bodies,
            mut embeddings,
            mut attachments
        } = self;

        //FIXME[rust/catch block] use catch block
        let catch_res = (|| -> Result<_, Error> {
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

            Ok((subject, bodies))
        })();

        let (subject, bodies) =
            match catch_res {
                Ok(vals) => vals,
                Err(err) => { return Either::B(future::err(err)); }
            };

        let loading_fut = Resource::load_container(embeddings, ctx)
            .join(Resource::load_container(attachments, ctx));

        let fut = loading_fut
            .map_err(Error::from)
            .map(|(embeddings, attachments)| {
                let inner = InnerTemplate {
                    template_name,
                    base_dir,
                    subject,
                    bodies,
                    embeddings,
                    attachments,
                    engine
                };

                Template { inner: Arc::new(inner) }
            });

        Either::A(fut)
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

pub fn deserialize_embeddings<'de, D>(deserializer: D)
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

pub fn deserialize_attachments<'de, D>(deserializer: D)
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

//TODO make base dir default to the dir the template file is in if it's parsed from a template file.

/// Common implementation for a type for [`TemplateEngine::LazyBodyTemplate`].
///
/// This impl. gives bodies a field `embeddings` which is a mapping of embedding
/// names to embeddings (using `deserialize_embeddings`) a `iri` field which
/// allows specifying the template (e.g. `"path:body.html"`) and can be relative
/// to the base dir. It also allows just specifying a string as template which defaults
/// to using `iri = "path:<thestring>"` and no embeddings.
#[derive(Debug, Serialize)]
pub struct StandardLazyBodyTemplate {
    pub iri: IRI,
    pub embeddings: HashMap<String, Resource>
}


impl PathRebaseable for StandardLazyBodyTemplate {
    fn rebase_to_include_base_dir(&mut self, base_dir: impl AsRef<Path>)
        -> Result<(), UnsupportedPathError>
    {
        self.iri.rebase_to_include_base_dir(base_dir)
    }

    fn rebase_to_exclude_base_dir(&mut self, base_dir: impl AsRef<Path>)
        -> Result<(), UnsupportedPathError>
    {
        self.iri.rebase_to_exclude_base_dir(base_dir)
    }
}


#[derive(Deserialize)]
#[serde(untagged)]
enum StandardLazyBodyTemplateDeserializationHelper {
    ShortForm(String),
    LongForm {
        iri: IRI,
        #[serde(default)]
        #[serde(deserialize_with="deserialize_embeddings")]
        embeddings: HashMap<String, Resource>
    }
}

impl<'de> Deserialize<'de> for StandardLazyBodyTemplate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        use self::StandardLazyBodyTemplateDeserializationHelper::*;
        let helper = StandardLazyBodyTemplateDeserializationHelper::deserialize(deserializer)?;
        let ok_val =
            match helper {
                ShortForm(string) => {
                    //UNWRAP_SAFE: only scheme can fail but is known to be ok
                    let iri = IRI::from_parts("path", &string).unwrap();
                    StandardLazyBodyTemplate {
                        iri,
                        embeddings: Default::default()
                    }
                },
                LongForm {iri, embeddings} => StandardLazyBodyTemplate { iri, embeddings }
            };
        Ok(ok_val)
    }
}


#[cfg(test)]
mod test {

    #[allow(non_snake_case)]
    mod StandardLazyBodyTemplate {

        use serde::{Serialize, Deserialize};
        use toml;

        use mail_core::Resource;

        use super::super::StandardLazyBodyTemplate;

        #[derive(Serialize, Deserialize)]
        struct Wrapper {
            body: StandardLazyBodyTemplate
        }

        #[test]
        fn should_deserialize_from_string() {
            let toml_str = r#"
                body = "template.html.hbs"
            "#;

            let Wrapper { body } = toml::from_str(toml_str).unwrap();
            assert_eq!(body.iri.as_str(), "path:template.html.hbs");
            assert_eq!(body.embeddings.len(), 0);
        }

        #[test]
        fn should_deserialize_from_object_without_embeddings() {
            let toml_str = r#"
                body = { iri="path:t.d" }
            "#;

            let Wrapper { body }= toml::from_str(toml_str).unwrap();
            assert_eq!(body.iri.as_str(), "path:t.d");
            assert_eq!(body.embeddings.len(), 0);
        }

        #[test]
        fn should_deserialize_from_object_with_empty_embeddings() {
            let toml_str = r#"
                body = { iri="path:t.d", embeddings={} }
            "#;

            let Wrapper { body } = toml::from_str(toml_str).unwrap();
            assert_eq!(body.iri.as_str(), "path:t.d");
            assert_eq!(body.embeddings.len(), 0);
        }

        #[test]
        fn should_deserialize_from_object_with_short_from_embeddings() {
            let toml_str = r#"
                body = { iri="path:t.d", embeddings={ pic1="the_embeddings" } }
            "#;

            let Wrapper { body } = toml::from_str(toml_str).unwrap();
            assert_eq!(body.iri.as_str(), "path:t.d");
            assert_eq!(body.embeddings.len(), 1);

            let (key, resource) = body.embeddings.iter().next().unwrap();
            assert_eq!(key, "pic1");

            if let &Resource::Source(ref source) = resource {
                assert_eq!(source.iri.as_str(), "path:the_embeddings");
            } else { panic!("unexpected resource: {:?}", resource)}
        }
    }
}
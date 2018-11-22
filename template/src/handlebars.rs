use hbs;
use serde::{Serialize, Deserialize};
use galemu::{Bound, BoundExt, create_gal_wrapper_type, Ref};
use failure::Error;


use super::{
    TemplateEngine,
    TemplateEngineCanHandleData,
    BodyTemplate,
    PreparationData,
    AdditionalCIds,
    PathRebaseable,
    UnsupportedPathError,
    serde_impl
};



pub struct Handlebars {
    inner: hbs::Handlebars,
    name_counter: usize
}

impl Handlebars {

    fn next_body_template_name(&mut self) -> String {
        let name = format!("body_{}", self.name_counter);
        self.name_counter += 1;
        name
    }
}
impl TemplateEngine for Handlebars {
    type Id = String;

    type LazyBodyTemplate = serde_impl::StandardLazyBodyTemplate;

    fn load_body_template(&mut self, tmpl: Self::LazyBodyTemplate)
        -> Result<BodyTemplate<Self>, Error>
    {
        let StandardLazyBodyTemplate {
            path, embeddings, media_type
        } = tmpl;

        let name = self.next_body_template_name();
        self.inner.register_template_file(name, path)?;

        let media_type =
            if let Some(media_type) = media_type {
                media_type
            } else if let Some(extension) = path.extension().and_then(|osstr| osstr.as_str()) {
                match extension {
                    "html" => "text/html; charset=utf-8".parse().unwrap(),
                    "txt" => "text/plain; charset=utf-8".parse().unwrap()
                }
            } else {
                return Err(failure::err_msg(
                    "handlebars requires html/txt file extension or media type given in template spec"
                ));
            };

        Ok(BodyTemplate {
            template_id: name,
            media_type: TODO,
            inline_embeddings: Default::default(),
        })
    }

    fn load_subject_template(&mut self, template_string: String)
        -> Result<Self::Id, Error>
    {
        Ok(self.inner.register_template_string("subject".to_owned(), template_string)?)
    }
}

/// Additional trait a template engine needs to implement for the types it can process as input.
///
/// This could for example be implemented in a wild card impl for the template engine for
/// any data `D` which implements `Serialize`.
impl<D> TemplateEngineCanHandleData<D> for Handlebars
    where D: Serialize
{
    fn render<'r, 'a>(
        &'r self,
        id: &'r Self::Id,
        data: &'r D,
        additional_cids: AdditionalCIds<'r>
    ) -> Result<String, Error> {
        Ok(self.inner.render(id, SerHelper { data, cids: additional_cid })?)
    }
}

struct SerHelper<'r, D> {
    data: &'r D,
    cids: AdditionalCIds<'r>
}
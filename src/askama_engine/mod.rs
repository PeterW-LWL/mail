use vec1::Vec1;

use askama;
use headers::components::MediaType;
use mail::{Resource, Context};

use ::{TemplateEngine, MailParts, BodyPart, EmbeddedWithCId};
mod error;
pub use self::error::*;

pub trait AskamaMailTemplate: askama::Template {

    fn media_type(&self) -> MediaType;

    /// Implement this to have alternate bodies, e.g. a alternate text body for an html body
    ///
    /// A simple way to bind another template to an data type is by wrapping a reference of
    /// the original type into it.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[macro_use]
    /// # extern crate askama;
    /// # extern crate mail_template;
    /// # extern crate mail_headers;
    /// # mod mail { pub use mail_template::askama_engine as askama; pub use mail_headers::components::MediaType; }
    /// use std::ops::Deref;
    /// use std::borrow::Cow;
    /// use askama::Template;
    /// use mail::askama::AskamaMailTemplate;
    /// use mail::MediaType;
    ///
    /// #[derive(Template)]
    /// #[template(source = "<h2>Hy {{ name }}</h2>", ext="html")]
    /// struct HtmlHy {
    ///     name: &'static str
    /// }
    ///
    /// impl AskamaMailTemplate for HtmlHy {
    ///     fn media_type(&self) -> MediaType {
    ///         MediaType::parse("text/html; charset=utf-8").unwrap()
    ///     }
    ///
    ///     fn alternate_template<'a>(&'a self) -> Option<Box<AskamaMailTemplate + 'a>> {
    ///         // theoretically we could circumvent the boxing by returning a &Trait
    ///         // but this would require transmuting `&HtmlHy` to `&TextHy` so we don't
    ///         // do this
    ///         Some(Box::new(TextHy(self)))
    ///     }
    /// }
    ///
    /// #[derive(Template)]
    /// #[template(source = "Hy {{ name }}, use html please", ext="txt")]
    /// struct TextHy<'a>(&'a HtmlHy);
    ///
    /// /// we implement deref so that we can use the fields
    /// /// of `HtmlHy` without indirection, e.g. use `name`
    /// /// instead of `inner.name`
    /// impl<'a> Deref for TextHy<'a> {
    ///     type Target = HtmlHy;
    ///
    ///     fn deref(&self) -> &Self::Target {
    ///         self.0
    ///     }
    /// }
    ///
    /// impl<'a> AskamaMailTemplate for TextHy<'a> {
    ///     fn media_type(&self) -> MediaType {
    ///         MediaType::parse("text/plain; charset=utf-8").unwrap()
    ///     }
    /// }
    ///
    /// fn main() {
    ///     let hy = HtmlHy { name: "Liz" };
    ///
    ///     let rendered = hy.render().unwrap();
    ///     assert_eq!(rendered, "<h2>Hy Liz</h2>");
    ///     let rendered = hy.alternate_template().unwrap().render().unwrap();
    ///     assert_eq!(rendered, "Hy Liz, use html please");
    /// }
    /// ```
    ///
    fn alternate_template<'a>(&'a self) -> Option<Box<AskamaMailTemplate + 'a>> {
        None
    }

    fn attachments(&self) -> Vec<Resource> {
        Vec::new()
    }
}


pub struct AskamaTemplateEngine;


impl<C, D> TemplateEngine<C, D> for AskamaTemplateEngine
    where C: Context, D: AskamaMailTemplate
{
    type TemplateId = ();
    type Error = AskamaError;

    fn use_template(&self, _id: &(), data: &D, ctx: &C) -> Result<MailParts, Self::Error> {
        let mut state = State::new(ctx);
        state.render_bodies::<Self::Error>(data)?;
        let (alternative_bodies, attachments) = state.destruct();

        Ok(MailParts {
            alternative_bodies,
            attachments,
            shared_embeddings: Vec::new(),
        })
    }
}

struct State<'a, C: 'a> {
    ctx: &'a C,
    bodies: Vec<BodyPart>,
    attachments: Vec<EmbeddedWithCId>
}


impl<'a, C: 'a> State<'a, C>
    where C: Context
{
    fn new(ctx: &'a C) -> Self {
        State {
            ctx,
            bodies: Vec::new(),
            attachments: Vec::new()
        }
    }

    fn render_bodies<E>(
        &mut self,
        template: &AskamaMailTemplate,
    ) -> Result<(), E>
        where E: From<askama::Error>
    {
        let string = template.render()?;
        let media_type = template.media_type();
        let resource = Resource::sourceless(media_type, string);
        self.bodies.push(BodyPart {
            body_resource: resource,
            embeddings: Vec::new()
        });

        for attachment in template.attachments() {
            self.attachments.push(EmbeddedWithCId::attachment(attachment, self.ctx));
        }

        let sub = template.alternate_template();
        if let Some(alt) = sub {
            self.render_bodies::<E>(&*alt)?;
        }
        Ok(())
    }

    /// # Panics
    ///
    /// if render_bodies was not called at last once successfully
    fn destruct(self) -> (Vec1<BodyPart>, Vec<EmbeddedWithCId>) {
        let State { bodies, attachments, ctx:_ } = self;
        let bodies = Vec1::from_vec(bodies).expect("[BUG] should have at last one body");
        (bodies, attachments)
    }
}


#[cfg(test)]
mod test {

    use std::ops::Deref;
    use askama::Template;
    use super::*;
    //TODO test with alternate bodies and attachments

    struct Person {
        name: &'static str,
        name_prefix: &'static str
    }

    #[derive(Template)]
    #[template(source="<h2>Dear {{name_prefix}} {{name}}</h2>", ext="html")]
    // #[askama_mail(media_type = "text/html; charset=utf-8")]
    // #[askama_mail(alternate=TextGreeting)]
    struct HtmlGreeting<'a> {
        person: &'a mut Person
    }

    impl<'a> Deref for HtmlGreeting<'a> {
        type Target = Person;

        fn deref(&self) -> &Self::Target {
            self.person
        }
    }

    impl<'a> AskamaMailTemplate for HtmlGreeting<'a> {
        fn media_type(&self) -> MediaType {
            MediaType::parse("text/html; charset=utf-8").unwrap()
        }

        fn attachments(&self) -> Vec<Resource> {
            vec![ Resource::sourceless_from_string("hy"), Resource::sourceless_from_string("ho") ]
        }

        fn alternate_template<'e>(&'e self) -> Option<Box<AskamaMailTemplate + 'e>> {
            Some(Box::new(TextGreeting::from(self)))
        }
    }



    #[derive(Template)]
    #[template(source="Dear {{name_prefix}} {{name}}", ext="txt")]
    // #[askama_mail(media_type = "text/plain; charset=utf-8")]
    // #[askama_mail(wraps=HtmlGreeting)]
    struct TextGreeting<'a> {
        inner: &'a HtmlGreeting<'a>
    }

    impl<'a> AskamaMailTemplate for TextGreeting<'a> {
        fn media_type(&self) -> MediaType {
            MediaType::parse("text/plain; charset=utf-8").unwrap()
        }

        fn attachments(&self) -> Vec<Resource> {
            vec![ Resource::sourceless_from_string("so") ]
        }
    }

    //auto-gen from wraps
    impl<'a> Deref for TextGreeting<'a> {
        type Target = HtmlGreeting<'a>;

        fn deref(&self) -> &Self::Target {
            self.inner
        }
    }

    //auto-gen from wraps
    impl<'a> From<&'a HtmlGreeting<'a>> for TextGreeting<'a> {
        fn from(inner: &'a HtmlGreeting<'a>) -> Self {
            TextGreeting { inner }
        }
    }

}
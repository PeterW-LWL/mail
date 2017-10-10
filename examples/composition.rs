extern crate mail_codec;
extern crate futures;
extern crate mime;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use futures::Future;
use template_engine::Teng;

use mail_codec::composition_prelude::*;
use mail_codec::resource_prelude::*;

use mail_codec::default_impl::{ SimpleContext, NoNameComposer};

fn main() {
    _main().unwrap();
}

fn _main() -> Result<()> {
    let context = SimpleContext::new( "content_id_postfix.is.this".into() );
    let composer = Compositor::new( Teng::new(), context.clone(), NoNameComposer );

    let data = Resorts {
        listing: vec![
            Resort {
                name: "The Naping Lake".into(),
                image: fake_avatar(0),
                portfolio: Resource::from_text( "da naps'de weg".into() ).into(),
            },
            Resort {
                name: "Snoring Dog Tower".into(),
                image: fake_avatar(1),
                portfolio: Resource::from_text( "Our Motto: wau\r\nwau wau\r\nwau".into() ).into(),
            }
        ]
    };

    let from_to = MailSendContext {
        from: Email::from_input( "my@sender.yupyup" )?.into(),
        to: Email::from_input( "goblin@dog.spider" )?.into(),
        subject: "Dear randomness".into(),
    };

    let mail = composer.compose_mail(
        from_to,
        "the_template_id_ignored_in_this_examples",
        data,
    )?;

    let mut encoder = Encoder::new( MailType::Ascii );
    let encodable_mail = mail.into_future( &context ).wait().unwrap();
    encodable_mail.encode( &mut encoder )?;


    println!( "{}", encoder.into_string_lossy().unwrap() );

    Ok( () )
}


fn fake_avatar(nr: u32) -> Embedding {
    let mut resource = Resource::from_text(
        if nr == 0 {
            r#"this should be an image ¯\_(ツ)_/¯"#.into()
        } else {
            r#" another image..."#.into()
        }
    );
    resource.set_preferred_encoding( TransferEncoding::Base64 );
    Embedding::new( resource )
}

#[derive(Serialize)]
struct Resorts {
    listing: Vec<Resort>
}

#[derive(Serialize)]
struct Resort {
    name: String,
    image: Embedding,
    portfolio: Attachment
}


mod template_engine {
    use serde_json;
    use mail_codec::template_engine_prelude::*;

    /// Example template engine which turns everything into a json blob
    pub struct Teng;

    impl Teng {
        pub fn new() -> Self {
            Teng
        }
    }

    impl<C: Context> TemplateEngine<C> for Teng {
        type TemplateId = &'static str;
        type Error = serde_json::Error;

        fn templates<D: Serialize>(
            &self,
            _ctx: &C,
            _id: Self::TemplateId,
            data: D
        ) -> StdResult<Vec1<Template>, Self::Error> {
            // Note: we can use `_ctx` to if we really need to, e.g. to generate ContentID's,
            // through notice, that we can always use Embedding without a content ID
            // and the compositor will handle that part for us.
            // FIXME make it so that TemplateEngine can, but does not has to, know about the type of
            // context, to e.g. access configutations etc.

            // Note: while this example ignores `_id` a template engine normally determines which
            //  template(s) to use through the id

            let stringified = serde_json::to_string_pretty(&data)?.replace("\n", "\r\n");
            let text = format!(concat!(
                "This is the data the template engine gets in JSON:\r\n",
                "\r\n{}\r\n\r\n",
                "Note that the value for avatar is a ContentID,\r\n",
                " e.g. in a HTML body cid:theid can be used to refer to it\r\n",
                "Note that the value for signature is null because signature is\r\n",
                " an Attachment and as such there is no reason represent it in\r\n",
                " the data, use an embedding if you want to refere to it/inline it\r\n"
            ), stringified);
            Ok(Vec1::new(Template {
                body: Resource::from_text(text),
                embeddings: Vec::new(),
                attachments: Vec::new(),
            }))
        }
    }
}
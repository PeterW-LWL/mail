//
// !! YOU MIGHT WANT TO TAKE A LOOK AT tests/tera/main.rs WHICH IS A BETTER EXAMPLE !!
//
extern crate mail_codec as mail;
extern crate mail_codec_composition as compose;
extern crate futures;
extern crate mime;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate futures_cpupool;

use std::borrow::Cow;

use futures_cpupool::{CpuPool, Builder as CpuPoolBuilder};
use futures::Future;

use template_engine::Teng;

use compose::error::Error;
use compose::composition_prelude::*;
use compose::resource_prelude::*;

use compose::default_impl::{RandomContentId, NoNameComposer};
use compose::CompositeContext;
use mail::default_impl::FsResourceLoader;
use mail::context::CompositeBuilderContext;

type MyContext =
CompositeContext<RandomContentId, CompositeBuilderContext<FsResourceLoader, CpuPool>>;

fn setup_context() -> MyContext {
    CompositeContext::new(
        RandomContentId::new("content_id_postfix.is.this"),
        CompositeBuilderContext::new(
            FsResourceLoader::with_cwd_root().unwrap(),
            CpuPoolBuilder::new().create()
        )
    )
}

fn main() {
    _main().unwrap();
}

fn _main() -> Result<(), Error> {
    let context = setup_context();
    let template_engine = Teng::new();

    let data = Resorts {
        listing: vec![
            Resort {
                name: "The Naping Lake".into(),
                image: fake_avatar(0),
                portfolio: fake_resource("da naps'de weg").into(),
            },
            Resort {
                name: "Snoring Dog Tower".into(),
                image: fake_avatar(1),
                portfolio: fake_resource("Our Motto: wau\r\nwau wau\r\nwau").into(),
            }
        ]
    };

    let mut send_data = MailSendData::simple_new(
        Email::try_from( "my@sender.yupyup" )?.into(),
        Email::try_from( "goblin@dog.spider" )?.into(),
        "Dear randomness",
        Cow::Borrowed("the_template_id_ignored_in_this_examples"),
        data
    );

    //this doesn't realy do anything as the NoNameComposer is used
    send_data.auto_gen_display_names(NoNameComposer)?;

    let mail = (&context, &template_engine).compose_mail(send_data)?;

    let mut encoder = Encoder::new( MailType::Ascii );
    let encodable_mail = mail.into_encodeable_mail( &context ).wait().unwrap();
    encodable_mail.encode( &mut encoder )?;


    println!( "{}", encoder.to_string_lossy().unwrap() );

    Ok( () )
}

fn fake_resource(str: &str) -> Resource {
    use mail::file_buffer::FileBuffer;
    use mail::MediaType;
    let media_type = MediaType::parse("text/plain; charset=utf-8").unwrap();
    let fb = FileBuffer::new(media_type, str.as_bytes().to_owned());
    Resource::sourceless_from_buffer(fb)
}

fn fake_avatar(nr: u32) -> Embedding {
    let resource = fake_resource(
        if nr == 0 {
            r#"this should be an image ¯\_(ツ)_/¯"#.into()
        } else {
            r#" another image which isn't one..."#.into()
        }
    );
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
    use super::fake_resource;
    use serde_json;
    use compose::template_engine_prelude::*;

    /// Example template engine which turns everything into a json blob
    pub struct Teng;

    impl Teng {
        pub fn new() -> Self {
            Teng
        }
    }

    impl<C: Context> TemplateEngine<C> for Teng {
        type TemplateId = str;
        type Error = serde_json::Error;

        fn use_templates<D: Serialize>(
            &self,
            _ctx: &C,
            _id: &Self::TemplateId,
            data: &D
        ) -> Result<MailParts, Self::Error> {
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
            let bodies = Vec1::new(BodyPart {
                body_resource: fake_resource(&text),
                embeddings: Default::default(),
            });
            Ok(MailParts {
                alternative_bodies: bodies,
                shared_embeddings: Vec::new(),
                attachments: Vec::new()
            })
        }
    }
}
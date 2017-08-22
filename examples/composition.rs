#[macro_use(sep_for)]
extern crate mail_codec;
extern crate futures;
extern crate mime;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use std::result::{ Result as StdResult };

use futures::Future;

use serde::Serialize;

use mail_codec::types::Vec1;
use mail_codec::error::*;
use mail_codec::grammar::MailType;
use mail_codec::components::{
    Email,
    TransferEncoding
};
use mail_codec::data::FromInput;
use mail_codec::codec::{
    MailEncodable,
    MailEncoderImpl
};
use mail_codec::mail::{
    Resource,
};
use mail_codec::composition::{
    TemplateEngine,
    Template,
    Compositor,
    Context,
    NameComposer,
    MailSendContext,
    Embedding, Attachment
};

use mail_codec::default_impl::{
    SimpleContext
};

fn main() {
    _main().unwrap();
}

fn _main() -> Result<()> {
    let context = SimpleContext::new( "content_id_postfix.is.this".into() );
    let composer = create_composer( &context );

    let data = Users {
        users: vec![
            User {
                name: "Goblin the First".into(),
                avatar: fake_avatar(0),
                signature: Resource::from_text( "wha'da signatur?".into() ).into(),
            },
            User {
                name: "Spider Dog".into(),
                avatar: fake_avatar(1),
                signature: Resource::from_text( "wau\r\nwau wau\r\nwau".into() ).into(),
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

    let mut encoder = MailEncoderImpl::new( MailType::Ascii );
    let encodable_mail = mail.into_future( &context ).wait().unwrap();
    encodable_mail.encode( &mut encoder )?;

    let as_buff: Vec<u8> = encoder.into();

    //FIXME newline, between header and body
    println!( "{}", String::from_utf8_lossy( &*as_buff ) );

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
struct Users {
    users: Vec<User>
}

#[derive(Serialize)]
struct User {
    name: String,
    avatar: Embedding,
    signature: Attachment
}


struct NComp;

impl NameComposer<Users> for NComp {
    fn compose_name( &self, data: &Users ) -> Option<String> {
        let mut name = String::new();
        sep_for!{ user in data.users.iter();
            sep { name += " who is also known as " };
            name += &*user.name
        }
        if name.len() > 0 {
            Some( name )
        } else {
            None
        }
    }
}

type Composer = Compositor<Teng, SimpleContext, NComp, Users>;

fn create_composer( ctx: &SimpleContext ) -> Composer {
    //FIXME the Compositior is bound to the data type BUT ...
    // 1. is that needed
    // 2. how to make type inference work nicely with a new which DOES NOT LINK TO DATA
    Compositor::new( Teng::new(), ctx.clone(), NComp )
}



//TODO make a prelude alla:
// template_engine_prelude {
//      TemplateEngine, Template, StdError, StdResult, Serialize, Context
//      Vec1
// }
/// Example template engine which turns everything into a json blob
struct Teng;

impl Teng {

    pub fn new() -> Self {
        Teng
    }
}

impl TemplateEngine for Teng {
    type TemplateId = &'static str;
    type Error = serde_json::Error;

    fn templates<D: Serialize, C: Context>(
        &self,
        _ctx: &C,
        _id: Self::TemplateId,
        data: D
    ) -> StdResult< Vec1<Template>, Self::Error > {
        // Note: we can use `_ctx` to if we really need to, e.g. to generate ContentID's,
        // through notice, that we can always use Embedding without a content ID
        // and the compositor will handle that part for us.
        // FIXME make it so that TemplateEngine can, but does not has to, know about the type of
        // context, to e.g. access configutations etc.

        // Note: while this example ignores `_id` a template engine normally determines which
        //  template(s) to use through the id
        
        let stringified = serde_json::to_string_pretty( &data )?.replace( "\n", "\r\n" );
        let text = format!( concat!(
            "This is the data the template engine gets in JSON:\r\n",
            "\r\n{}\r\n\r\n",
            "Note that the value for avatar is a ContentID,\r\n",
            " e.g. in a HTML body cid:theid can be used to refer to it\r\n",
            "Note that the value for signature is null because signature is\r\n",
            " an Attachment and as such there is no reason represent it in\r\n",
            " the data, use an embedding if you want to refere to it/inline it\r\n"
        ), stringified );
        Ok( Vec1::new( Template {
            body: Resource::from_text( text ),
            embeddings: Vec::new(),
            attachments: Vec::new(),
        }) )
    }

}

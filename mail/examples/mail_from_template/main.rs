//! In this example a mail is created using the tempalte engine with tera bindings
//! (and then printed)
//!
#[cfg(not(all(feature = "render-template-engine", feature = "tera-engine")))]
compile_error!("example `mail_from_template` requires feature `render-template-engine` and `tera-engine`");

#[macro_use]
extern crate mail;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate serde_derive;
extern crate futures;
extern crate soft_ascii_string;

use futures::Future;
use soft_ascii_string::SoftAsciiString;

use std::str;
use std::borrow::Cow;

use mail::error::MailError;
use mail::{Mail, Mailbox, MailType, HeaderTryFrom, Context, Domain};
use mail::default_impl::simple_context;
use mail::template::{InspectEmbeddedResources, Embedded, MailSendData};
use mail::render_template_engine::{RenderTemplateEngine, TemplateSpec, DEFAULT_SETTINGS};
use mail::tera::TeraRenderEngine;

use self::error::{SetupError, Error};

mod error;

#[derive(Debug, Serialize, InspectEmbeddedResources)]
struct UserData {
    name: &'static str
    // TODO: include embedded avatar.
}

fn main() {
    let msg_id_domain = Domain::try_from("company_a.test").unwrap();
    let unique_part = SoftAsciiString::from_string("c207n521cec").unwrap();
    let ctx = simple_context::new(msg_id_domain, unique_part).unwrap();

    let engine = setup_template_engine().unwrap();

    let mail = create_mail(&engine, &ctx).unwrap();
    let string_mail = encode_mail_to_string(mail, ctx.clone()).unwrap();
    println!("{}", string_mail);
}

fn create_mail(engine: &RenderTemplateEngine<TeraRenderEngine>, ctx: &impl Context)
    -> Result<Mail, Error>
{
    let from        = Mailbox::try_from("a@b.c")?;
    let to          = Mailbox::try_from("d@e.f")?;
    let subject     = "Live might not be a roller coaster";
    // Use template_a.
    let template_id = Cow::Borrowed("template_a");
    // This can be basically a type implementing `Serialize`,
    // in the tera template we use it with `{{data.name}}`.
    let data        = UserData { name: "Mr. Slow Coaster" };

    // `MailSendData` contains everything needed to crate (and send)
    // a mail based on a template.
    let send_data = MailSendData::simple_new(
        from, to, subject,
        template_id, data
    );

    let mail = send_data.compose(ctx, engine)?;
    Ok(mail)
}

fn setup_template_engine() -> Result<RenderTemplateEngine<TeraRenderEngine>, SetupError> {
    // Create instance of the tera rte implementation,
    // we can reuse/derive from all templates in `tera_base` with
    // this (and we do so in the example using `{% extends "base_mail.html" %}`).
    let tera = TeraRenderEngine::new("./example_resources/tera_base/**/*")?;
    let mut rte = RenderTemplateEngine::new(tera);
    // Load all template specs based on the files/folders in `templates`
    // using the folder structure as way to define the templates is easy
    // but we can do differently if we need to.
    let specs = TemplateSpec
        ::from_dirs("./example_resources/templates", &*DEFAULT_SETTINGS)?;

    for (name, spec) in specs {
        rte.insert_spec(name, spec)?;
    }
    Ok(rte)
}

fn encode_mail_to_string(mail: Mail, ctx: impl Context) -> Result<String, MailError> {
    let res = mail
        .into_encodeable_mail(ctx)
        .wait()?
        .encode_into_bytes(MailType::Ascii)?;

    Ok(String::from_utf8(res).unwrap())
}

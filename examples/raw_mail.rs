#[macro_use]
extern crate mail_codec;
extern crate futures;
extern crate mime;

use futures::{ future, Future };

use mail_codec::mail_builder_prelude::*;
use mail_codec::resource_prelude::*;

use mail_codec::default_impl::SimpleBuilderContext;



fn get_some_resource() -> Resource {
    let data: Vec<u8> = "abcd↓efg".as_bytes().to_vec();
    Resource::from_future(
        future::ok( FileBuffer::new( mime::TEXT_PLAIN, data ) ).boxed()
    )
}

fn main() {
    _main().unwrap();
}

fn _main() -> Result<()> {
    let mut encoder = MailEncoderImpl::new( MailType::Ascii );

    let builder_ctx = SimpleBuilderContext::default();
    let headers = headers! {
        //FIXME actually use a more realistic header setup
        Subject: "that ↓ will be encoded ",
        MessageId: "ran.a1232.13rwqf23.a@dom",
        From: [
            ("random dude", "this@is.es"),
            ("another person", "abc@def.geh")
        ],
        ReturnPath: None
    }?;
    let mail = Builder::singlepart( get_some_resource() )
        .headers( headers )?
        .build()?;

    let encodable_mail = mail.into_future( &builder_ctx ).wait().unwrap();
    encodable_mail.encode( &mut encoder )?;

    let as_buff: Vec<u8> = encoder.into();

    //FIXME newline, between header and body
    println!( "{}", String::from_utf8_lossy( &*as_buff ) );

    Ok( () )


}
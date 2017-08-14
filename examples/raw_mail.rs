#[macro_use]
extern crate mail_codec;
extern crate futures;
extern crate mime;

use futures::{ future, Future, IntoFuture };

use mail_codec::error::*;

use mail_codec::types::buffer::FileBuffer;
use mail_codec::mail::Builder;
use mail_codec::mail::resource::Resource;
use mail_codec::mail::test_utils::TestBuilderContext;
use mail_codec::codec::MailEncodable;

use mail_codec::codec::MailEncoderImpl;

use mail_codec::data::FromInput;
use mail_codec::headers::Header;
use mail_codec::components::*;
use mail_codec::grammar::MailType;

fn get_some_resource() -> Resource {
    let data: Vec<u8> = "abcd↓efg".as_bytes().to_vec();
    Resource::Future(
        future::ok( FileBuffer::new( mime::TEXT_PLAIN, data ) ).boxed()
    )
}

fn main() {
    _main().unwrap();
}

fn _main() -> Result<()> {
    let mut encoder = MailEncoderImpl::new( MailType::Ascii );

    let builder_ctx = TestBuilderContext::default();

    let mail = Builder(builder_ctx).singlepart( get_some_resource() )
        .set_header(
            Header::Subject(
                Unstructured::from_input( "that ↓ will be encoded ")? ) )?
        .set_header(
            //FIXME check why this is invalid
            Header::MessageID( MessageID::from_input( "ran.a1232.13rwqf23.a@dom" )? )
        )?
        .set_header(
            Header::From( MailboxList( vec1![
                Mailbox {
                    display_name: Some( Phrase::from_input( "random dude" )? ),
                    email: Email::from_input( "this@is.es" )?
                },
                Mailbox {
                    display_name: Some( Phrase::from_input( "random dude" )? ),
                    email: Email::from_input( "this@is.es" )?
                }
            ]))
        )?
        .set_header(
            Header::ReturnPath( Path( None ) )
        )?
        .build()?;

    let encodable_mail = mail.into_future().wait().unwrap();
    encodable_mail.encode( &mut encoder )?;

    let as_buff: Vec<u8> = encoder.into();

    //FIXME newline, between header and body
    println!( "{}", String::from_utf8_lossy( &*as_buff ) );

    Ok( () )


}
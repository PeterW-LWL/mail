extern crate mail_codec;
extern crate futures;
extern crate mime;

use futures::{ future, Future, IntoFuture };

use mail_codec::error::*;

use mail_codec::types::buffer::FileBuffer;
use mail_codec::mail::Builder;
use mail_codec::mail::resource::Resource;
use mail_codec::default_impl::SimpleBuilderContext;
use mail_codec::codec::MailEncodable;

use mail_codec::codec::MailEncoderImpl;

use mail_codec::data::FromInput;
use mail_codec::headers::Header;
use mail_codec::components::Unstructured;
use mail_codec::grammar::MailType;

use mail_codec::mail::mime::MultipartMime;

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

    let builder_ctx = SimpleBuilderContext::default();


    let mail = Builder(builder_ctx).multipart(
            MultipartMime::new( "multipart/related; boundary=\"=_abc\"".parse().unwrap() )? )
        .set_header(Header::Subject( Unstructured::from_input( "that ↓ will be encoded ")? ) )?
        .add_body( |bb| bb.singlepart( get_some_resource() ).build() )?
        .add_body( |bb| bb.singlepart( get_some_resource() ).build() )?
        .build()?;



    let encodable_mail = mail.into_future().wait().unwrap();
    encodable_mail.encode( &mut encoder )?;

    let as_buff: Vec<u8> = encoder.into();

    //FIXME newline, between header and body
    println!( "{}", String::from_utf8_lossy( &*as_buff ) );

    Ok( () )


}
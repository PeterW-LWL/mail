use super::*;
use mime::BOUNDARY;
use ascii::IntoAsciiString;
use headers::HeaderName;

use headers::{
    ContentType,
    ContentTransferEncoding
};

///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
#[inline(always)]
pub fn encode_mail<E>( mail: &EncodableMail<E>, top: bool, encoder: &mut E ) -> Result<()>
    where E: MailEncoder
{
    _encode_mail( &mail.0, top, encoder )
}

fn _encode_mail<E>( mail: &Mail<E>, top: bool, encoder: &mut E ) -> Result<()>
    where E: MailEncoder
{
    encode_headers( &mail, top, encoder )?;

    //the empty line between the headers and the body
    encoder.write_new_line();

    encode_mail_part( &mail, encoder )?;

    Ok( () )
}

///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
fn encode_headers<E>(mail: &Mail<E>, top: bool, encoder:  &mut E ) -> Result<()>
    where E: MailEncoder
{

    if top {
        encoder.write_str( ascii_str!{ M I M E Minus V e r s i o n Colon Space _1 Dot _0 } );
        encoder.write_new_line();
    }

    //TODO we have to special handle some headers which
    // _should_ be at the beginning mainly `Trace` and `Resend-*`

    // also the Resend's are grouped in blocks ...
    // ... so I have to have some sored of grouping/ordering hint
    // ... also this is NOT a static property as you have to know
    //     which belong together
    // ... what can be part of the type is wether or not there
    //     might be an ordering
    // ... we also want to be able to access the information about
    //     if it needs special ordering without VCalls
    // ... we could have a wrapper type `WithOrdering(T, OrderInfo)`
    //     but we can not implicity add it
    // ... there information wether or not ther is/might be a ordering
    //     can be stored in the body so no VCall for it

    let header_override;
    match mail.body {
        MailPart::SingleBody { ref body } => {
            let file_buffer = body.get_if_encoded()?
                .expect( "encoded mail, should only contain already transferencoded resources" );

            // handle Content-Type/Transfer-Encoding <-> Resource link
            encode_header( encoder,
                           ContentType::name(), file_buffer.content_type() )?;
            encode_header( encoder,
                           ContentTransferEncoding::name(), file_buffer.transfer_encoding() )?;

            header_override = true;
        },
        _ => {
            header_override = false;
        }
    }


    for (name, hbody) in mail.headers.iter() {
        let name_as_str = name.as_str();
        let ignored_header = !top &&
            !(name_as_str.starts_with("Content-")
                || name_as_str.starts_with("X-") );

        if ignored_header {
            //TODO warn!
        }
        if header_override && ( &name == "Content-Type" || &name == "Content-Transfer-Encoding" ) {
            //TODO warn!? header will be overriden (but possible with same value)
            continue
        }

        encode_header( encoder, name, hbody)?;
    }
    Ok( () )
}

fn encode_header<E>( encoder: &mut E, name: HeaderName, component: &MailEncodable<E>) -> Result<()>
    where E: MailEncoder
{
    encoder.write_str( name.as_ascii_str() );
    encoder.write_char( AsciiChar::Colon );
    encoder.write_fws();
    component.encode( encoder )?;
    encoder.write_new_line();
    Ok( () )
}

///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
fn encode_mail_part<E>(mail: &Mail<E>, encoder:  &mut E ) -> Result<()>
    where E: MailEncoder
{
    use super::MailPart::*;
    match mail.body {
        SingleBody { ref body } => {
            if let Some( file_buffer ) = body.get_if_encoded()? {
                encoder.write_body( &*file_buffer );
                encoder.write_new_line();
            } else {
                bail!( "encoded mail, should only contain already transferencoded resources" )
            }
        },
        MultipleBodies { ref hidden_text, ref bodies } => {
            if hidden_text.len() > 0 {
                //TODO warn that encoding hidden text is not implemented for now
            }
            let boundary: String = {
                //FIXME there has to be a better way
                if let Some( mime ) = mail.headers.get_single::<ContentType>()? {
                    mime.get_param(BOUNDARY)
                        .ok_or_else( ||-> Error { "boundary gone missing".into() } )?
                        .to_string()
                } else {
                    bail!( "Content-Type header gone missing" );
                }
            };

            let boundary = boundary.into_ascii_string().chain_err( || "non ascii boundary" )?;

            for mail in bodies.iter() {
                encoder.write_char( AsciiChar::Minus );
                encoder.write_char( AsciiChar::Minus );
                encoder.write_str( &*boundary );
                encoder.write_new_line();

                _encode_mail( mail, false, encoder )?;
            }

            if bodies.len() > 0 {
                encoder.write_char( AsciiChar::Minus );
                encoder.write_char( AsciiChar::Minus );
                encoder.write_str( &*boundary );
                encoder.write_char( AsciiChar::Minus );
                encoder.write_char( AsciiChar::Minus );
                encoder.write_new_line();
            } else {
                //TODO warn
            }

        }
    }
    Ok( () )
    }

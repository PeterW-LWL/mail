use super::*;
use mime::BOUNDARY;
use ascii::IntoAsciiString;
use headers::HeaderName;

///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
#[inline(always)]
pub fn encode_mail(
    mail: &EncodableMail,
    top: bool,
    encoder: &mut Encoder<Resource>
) -> Result<()> {
    _encode_mail( &mail.0, top, encoder )
}

fn _encode_mail(
    mail: &Mail,
    top: bool,
    encoder: &mut Encoder<Resource>
) -> Result<()> {
    encode_headers( &mail, top, encoder )?;

    //the empty line between the headers and the body
    encoder.add_blank_line();

    encode_mail_part( &mail, encoder )?;

    Ok( () )
}

///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
fn encode_headers(
    mail: &Mail,
    top: bool,
    encoder:  &mut Encoder<Resource>
) -> Result<()> {
    let mut handle = encoder.encode_header_handle();
    if top {
        handle.write_str( ascii_str!{ M I M E Minus V e r s i o n Colon Space _1 Dot _0 } )?;
        handle.finish();
    }

    for (name, hbody) in mail.headers.iter() {
        let name_as_str = name.as_str();
        let ignored_header = !top &&
            !(name_as_str.starts_with("Content-")
                || name_as_str.starts_with("X-") );

        if ignored_header {
            //TODO warn!
        }

        encode_header( &mut handle, name, hbody)?;
    }
    Ok( () )
}

fn encode_header(
    handle: &mut EncodeHeaderHandle,
    name: HeaderName,
    component: &EncodableInHeader
) -> Result<()> {
    handle.write_str( name.as_ascii_str() )?;
    handle.write_char( AsciiChar::Colon )?;
    handle.write_fws();
    component.encode( handle )?;
    handle.finish();
    Ok( () )
}

///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
fn encode_mail_part(mail: &Mail, encoder:  &mut Encoder<Resource> ) -> Result<()> {
    use super::MailPart::*;
    match mail.body {
        SingleBody { ref body } => {
            //Note: Resource is a Arc so sheap to clone
            encoder.add_body(body.clone());
        },
        MultipleBodies { ref hidden_text, ref bodies } => {
            if hidden_text.len() > 0 {
                //TODO warn that encoding hidden text is not implemented for now
            }
            let boundary: String = {
                //FIXME there has to be a better way
                // yes if the boundary is missing just genrate one!
                if let Some( mime ) = mail.headers.get_single(ContentType) {
                    mime?.get_param(BOUNDARY)
                        .ok_or_else( ||-> Error { "boundary gone missing".into() } )?
                        .to_string()
                } else {
                    bail!( "Content-Type header gone missing" )
                }
            };

            let boundary = boundary.into_ascii_string()
                .chain_err( || "non ascii boundary" )?;

            for mail in bodies.iter() {
                {
                    let mut handle = encoder.encode_header_handle();
                    handle.write_char( AsciiChar::Minus )?;
                    handle.write_char( AsciiChar::Minus )?;
                    handle.write_str( &*boundary )?;
                    handle.finish();
                }
                _encode_mail( mail, false, encoder )?;
            }

            if bodies.len() > 0 {
                let mut handle = encoder.encode_header_handle();
                handle.write_char( AsciiChar::Minus )?;
                handle.write_char( AsciiChar::Minus )?;
                handle.write_str( &*boundary )?;
                handle.write_char( AsciiChar::Minus )?;
                handle.write_char( AsciiChar::Minus )?;
                handle.finish();
            } else {
                //TODO warn
            }

        }
    }
    Ok( () )
}

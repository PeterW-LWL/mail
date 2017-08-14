use super::*;
use mime::BOUNDARY;
use ascii::IntoAsciiString;


///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
pub fn encode_mail<E>( mail: &Mail, top: bool, encoder: &mut E ) -> Result<()>
    where E: MailEncoder
{

    encode_headers( mail, top, encoder )?;

    //the empty line between the headers and the body
    encoder.write_new_line();

    encode_mail_part( mail, encoder )?;

    Ok( () )
}

///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
pub fn encode_headers<E>(mail: &Mail, top: bool, encoder:  &mut E ) -> Result<()>
    where E: MailEncoder
{
    let special_headers = find_special_headers( mail );
    let iter = special_headers
        .iter()
        .chain( mail.headers.values() );

    if top {
        encoder.write_str( ascii_str!{ M I M E Minus V e r s i o n Colon Space _1 Dot _0 } );
        encoder.write_new_line();
    }

    for header in iter {
        let ignored_header = !top &&
            !(header.name().as_str().starts_with("Content-")
                || header.name().as_str().starts_with("X-") );

        if ignored_header {

            //TODO warn!
        }

        header.encode( encoder )?;
        encoder.write_new_line();
    }
    Ok( () )
}

//FEATURE_TODO(use_impl_trait): return impl Iterator or similar
///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
pub fn find_special_headers( mail: &Mail ) -> Vec<Header> {
    let mut headers = vec![];
    //we need: ContentType, ContentTransferEncoding, and ??
    match mail.body {
        MailPart::SingleBody { ref body } => {
            let file_buffer = body.file_buffer_ref().expect( "the body to be resolved" );
            headers.push(
                Header::ContentType( file_buffer.content_type().clone() ) );
            headers.push(
                Header::ContentTransferEncoding( file_buffer.transfer_encoding().clone() ) );
        },
        //TODO are there more special headers? (Such which are derived from the body, etc.)
        // yes if there are file_meta we want to replace any ContentDisposition header with
        // our version containing file meta
        //TODO bail if there is a ContentTransferEncoding in a multipart body!
        _ => {}
    }
    headers
}

///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
pub fn encode_mail_part<E>(mail: &Mail, encoder:  &mut E ) -> Result<()>
    where E: MailEncoder
{
    use super::MailPart::*;
    match mail.body {
        SingleBody { ref body } => {
            if let Some( file_buffer ) = body.file_buffer_ref() {
                encoder.write_body( file_buffer );
                encoder.write_new_line();
            } else {
                bail!( "unresolved body" )
            }
        },
        MultipleBodies { ref hidden_text, ref bodies } => {
            if hidden_text.len() > 0 {
                //TODO warn that encoding hidden text is not implemented for now
            }
            let boundary: String = {
                //FIXME there has to be a better way
                if let Some( header ) = mail.headers.get(
                    ascii_str!( C o n t e n t Minus T y p e )
                ) {
                    match header {
                        &Header::ContentType( ref mime ) => {
                            mime.get_param(BOUNDARY)
                                .ok_or_else( ||-> Error { "boundary gone missing".into() } )?
                                .to_string()
                        }
                        _ => bail!( "Content-Type header corrupted" )
                    }
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

                encode_mail( mail, false, encoder )?;
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

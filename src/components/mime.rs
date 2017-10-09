use std::error::Error;
use std::fmt;
use std::borrow::Cow;

use mime;
use ascii::AsciiStr;

use error::*;
use utils::HeaderTryFrom;
use grammar::{ is_token, MailType};
use codec::{ MailEncoder, MailEncodable, self };

pub use mime::Mime;


pub fn create_mime_parameters<I,K,V>(params: I, buf: &mut String, tp: MailType) -> Result<()>
    where I: IntoIterator<Item=(K, V)>, K: AsRef<str>, V: AsRef<str>
{
    for (name, value) in params.into_iter() {
        create_encoded_mime_parameter(name, value, buf, tp)?;
    }
    Ok(())
}

pub fn create_encoded_mime_parameter<K,V>(
        name: K,
        value: V,
        buf: &mut String,
        tp: MailType
    ) -> Result<()>
    where K: AsRef<str>, V: AsRef<str>
{
    let name = name.as_ref();
    assure_token(name)?;
    let value = value.as_ref();

    let res = codec::quoted_string::quote_if_needed(value, codec::quoted_string::TokenCheck, tp);
    let (value, needed_encoding) =
        if let Ok( (got_tp, res) ) = res  {
            debug_assert!( !(tp==MailType::Ascii && got_tp==MailType::Internationalized) );
            (res, false)
        } else {
            //to_owned as it is owned anyway (else quote if needed would have
            // returned Cow::Borrow)
            let value = match codec::mime::percent_encode_param_value(value) {
                Cow::Owned(owned) => owned,
                // we only end up here is no chare needed percent encoding,
                // but we only use percent encoding is at last one char does
                // need it
                //TODO make into a warning
                _ => unreachable!("[BUG] program failed to decide if or if not percent encoding is needed")
            };
            (Cow::Owned(value.into()), true)
        };

    buf.push(';');
    buf.push_str(name);
    if needed_encoding {
        buf.push('*');
    }
    buf.push('=');
    if needed_encoding {
        buf.push_str("utf8''");
    }
    buf.push_str(&*value);
    Ok(())
}

pub fn create_mime<T, ST, I, K, V>(_type: T, subtype: ST, params: I, mt: MailType)
    -> Result<mime::Mime>
    where T: AsRef<str>, ST: AsRef<str>,
          I: IntoIterator<Item=(K, V)>, K: AsRef<str>, V: AsRef<str>
{
    let mut string = String::from(assure_token(_type.as_ref())?);
    string.push('/');
    string.push_str(assure_token(subtype.as_ref())?);
    create_mime_parameters(params, &mut string, mt)?;

    //UNWRAP_SAFE: we do not have a unsafe mime constructor so we have to parse
    //it even through it can not be invalid
    Ok( string.parse::<mime::Mime>().expect("[BUG] mime generator generated invalid mime") )
}

fn assure_token(s: &str) -> Result<&str> {
    if !is_token(s) {
        bail!("string {:?} is not a valid token", s);
    }
    Ok(s)
}

// as we are in the same package as the definition of HeaderTryFrom
// this is possible even with orphan rules
impl<'a> HeaderTryFrom<&'a str> for mime::Mime {
    fn try_from(val: &'a str) -> Result<Self> {
        val.parse()
            .map_err( |ferr| ErrorKind::ParsingMime( ferr ).into() )
    }
}


impl<E> MailEncodable<E> for mime::Mime where E: MailEncoder {

    fn encode(&self, encoder: &mut E) -> Result<()> {
        let res = self.to_string();
        if !encoder.try_write_utf8__(&*res).is_ok() {
            match AsciiStr::from_ascii(&*res) {
                Ok(asciied) => encoder.write_str( asciied ),
                Err(_err) => bail!("mime containining utf8 in ascii only mail")
            }
        }
        Ok( () )
    }
}


//UPSTREAM(mime): open an issue that FromStrError does not implement Error
#[derive(Debug)]
pub struct MimeFromStrError( pub mime::FromStrError );

impl fmt::Display for MimeFromStrError {
    fn fmt( &self, fter: &mut fmt::Formatter ) -> fmt::Result {
        <MimeFromStrError as fmt::Debug>::fmt( self, fter )
    }
}
impl Error for MimeFromStrError {
    fn description(&self) -> &str {
        "parsing mime from str failed"
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use codec::test_utils::*;

    ec_test!{simple,{
        let mime: Mime = "text/wtf;charset=utf8;random=alot".parse().unwrap();
        Some( mime )
    } => ascii => [
        LinePart("text/wtf;charset=utf8;random=alot")
    ]}

    #[test]
    fn mime_param_simple() {
        let params = &[
            ("time", "unknown"),
            ("brain", "missing")
        ];
        let mut out = String::new();
        let res = create_mime_parameters(params.iter().cloned(), &mut out, MailType::Ascii);
        assert_ok!(res);

        assert_eq!(
            ";time=unknown;brain=missing",
            out.as_str()
        )
    }

    #[test]
    fn mime_param_quoted() {
        let params = &[
            ("time", "unknown think"),
            ("brain", "missing")
        ];
        let mut out = String::new();
        let res = create_mime_parameters(params.iter().cloned(), &mut out, MailType::Ascii);
        assert_ok!(res);

        assert_eq!(
            ";time=\"unknown think\";brain=missing",
            out.as_str()
        )
    }

    #[test]
    fn mime_param_quoted_utf8() {
        let params = &[
            ("time", "unknown ü\""),
            ("brain", "missing")
        ];
        let mut out = String::new();
        let res = create_mime_parameters(
            params.iter().cloned(), &mut out, MailType::Internationalized);
        assert_ok!(res);

        assert_eq!(
            r#";time="unknown ü\"";brain=missing"#,
            out.as_str()
        )
    }

    #[test]
    fn mime_param_encoded_in_ascii_but_not_in_utf8() {
        let params = &[
            ("time", "unknown ü\""),
            ("brain", "missing")
        ];
        let mut out = String::new();
        let res = create_mime_parameters(params.iter().cloned(), &mut out, MailType::Ascii);
        assert_ok!(res);

        assert_eq!(
            r#";time*=utf8''unknown%20%C3%BC%22;brain=missing"#,
            out.as_str()
        )
    }

    #[test]
    fn mime_param_always_encoded() {
        let params = &[
            ("filename", "u\x01\x02ps"),
        ];
        let mut out = String::new();
        let res =create_mime_parameters(
            params.iter().cloned(), &mut out, MailType::Internationalized);
        assert_ok!(res);

        assert_eq!(
            ";filename*=utf8''u%01%02ps",
            out.as_str()
        )
    }
}
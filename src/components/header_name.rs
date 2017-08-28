use std::ops::Deref;

use ascii::{ AsciiString, AsciiStr };

use error::*;
use codec::{ MailEncodable, MailEncoder };

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
pub struct HeaderName( AsciiString );

impl HeaderName {
    pub fn new( name: String ) -> Result<HeaderName> {
        let mut ok = true;
        for char in name.chars() {
            match char {
                'a'...'z' |
                'A'...'Z' |
                '0'...'9' |
                '-' => {},
                _ => { ok = false; break; }
            };
        }
        if ok {
            Ok( HeaderName( unsafe { AsciiString::from_ascii_unchecked( name ) } ) )
        } else {
            Err(ErrorKind::InvalidHeaderName(name).into())
        }
    }
}


impl<E> MailEncodable<E> for HeaderName where E: MailEncoder {

    fn encode(&self, encoder: &mut E) -> Result<()> {
        encoder.write_str( &*self.0 );
        Ok( () )
    }
}

impl Deref for HeaderName {
    type Target = AsciiStr;
    fn deref( &self ) -> &AsciiStr {
        &*self.0
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use codec::test_utils::LinePart;

    #[test]
    fn new_valid() {
        let name = "Content-Randomization-Factor";
        let valid = HeaderName::new( name.into() ).unwrap();
        assert_eq!(
            HeaderName( AsciiString::from_ascii( name ).unwrap() ),
            valid
        )
    }

    #[test]
    fn new_invalid() {
        let name = "X@3";
        let res = HeaderName::new( name.into() );
        assert_eq!( false, res.is_ok() );
    }

    ec_test!{ encode_header_name, {
        HeaderName::new( "X-Random".into() )
    } => ascii => [
        LinePart( "X-Random" )
    ]}
}
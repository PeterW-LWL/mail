use std::ops::Deref;

use ascii::{ AsciiString, AsciiStr, AsciiChar };

use error::*;
use codec::{ MailEncodable, MailEncoder };

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


impl MailEncodable for HeaderName {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()>
        where E: MailEncoder
    {
        encoder.write_str( &*self.0 );
        encoder.write_char( AsciiChar::Colon );
        Ok( () )
    }
}

impl Deref for HeaderName {
    type Target = AsciiStr;
    fn deref( &self ) -> &AsciiStr {
        &*self.0
    }
}
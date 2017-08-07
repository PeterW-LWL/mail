use std::ops::Deref;

use ascii::AsciiString;

use error::*;
use grammar::{is_qtext, is_ascii, is_ws, is_vchar, MailType };
use grammar::quoted_word::is_quoted_word;
use codec::MailEncoder;

use super::simple_item::SimpleItem;
use super::inner_item::{ InnerAscii, InnerUtf8 };

#[derive( Debug, Clone, Hash, Eq, PartialEq )]
pub struct QuotedString( SimpleItem );


impl QuotedString {

    pub fn write_into<E>( encoder: &mut E, input: &str ) -> Result<()>
        where E: MailEncoder
    {
        //OPTIMIZE: do not unnecessarily allocate strings, but directly write to Encoder
        use self::SimpleItem::*;
        let quoted = QuotedString::quote( input )?;
        match *quoted {
            Ascii( ref inner ) => {
                encoder.write_str( &*inner );
            },
            Utf8( ref inner ) => {
                encoder.try_write_utf8( &*inner )?
            }
        }
        Ok( () )
    }

    pub fn parse( already_quoted: SimpleItem ) -> Result<Self> {
        if is_quoted_word( &*already_quoted, MailType::Internationalized ) {
            Ok( QuotedString( already_quoted ) )
        } else {
            bail!( "already quoted item is not actualy valid: {:?}", &*already_quoted );
        }
    }

    pub fn quote( input: &str ) -> Result<Self> {
        let mut ascii = true;
        let mut out = String::new();
        out.push( '"' );
        for char in input.chars() {
            if ascii { ascii = is_ascii( char ) }
            if is_qtext( char, MailType::Internationalized ) {
                out.push( char )
            } else {
                //NOTE: while quoting ws is possible it is not nessesary as
                // a quoted string can contain FWS, and only CRLF in a quoted
                // string are semantically invisible (meaning the WSP after
                // CRLF _is_ semantically visible)
                if is_vchar( char, MailType::Internationalized) || is_ws( char ) {
                    out.push( '\\' );
                    out.push( char );
                } else {
                    // char: 0-31
                    bail!( "can not quote char: {:?}", char );
                }
            }
        }
        out.push( '"' );
        if ascii {
            let asciied = unsafe { AsciiString::from_ascii_unchecked( out ) };
            Ok( QuotedString( asciied.into() ) )
        } else {
            Ok( QuotedString( SimpleItem::from_utf8( out.into() ) ) )
        }
    }

    ///
    /// # Panics
    /// panics if the Quoted string was internally not valid
    ///
    pub fn unquote( &self ) -> SimpleItem {
        let quoted: &str = &**self;
        if quoted.len() < 2 { panic!( "invalide quoted string" ) }

        let mut iter = quoted.chars();

        let quote = iter.next().unwrap();
        if quote != '"' { panic!( "invalid quoted string" ) }

        let mut out = String::with_capacity(quoted.len() - 2);

        loop {
            if let Some( ch ) = iter.next() {
                match ch {
                    '\\' => {
                        if let Some( next ) = iter.next() {
                            out.push( next )
                        } else {
                            panic!( "invalide quoted string" );
                        }
                    },
                    '"' => {
                        if iter.next().is_some() {
                            panic!( "invalide quoted string" );
                        } else {
                            break;
                        }
                    },
                    _ => {
                        out.push( ch );
                    }
                }
            } else {
                panic!( "invalide quoted string" )
            }
        }

        if self.is_ascii() {
            //SAFE: if we didn't head any non-ascii-utf8 then we can not get some by unquoting
            SimpleItem::Ascii( InnerAscii::Owned( unsafe {
                AsciiString::from_ascii_unchecked( out )
            }))
        } else {
            SimpleItem::Utf8( InnerUtf8::Owned( out ) )
        }
    }
}


impl Into<String> for QuotedString {

    fn into( self ) -> String {
        self.0.into()
    }
}


impl Deref for QuotedString {
    type Target = SimpleItem;

    fn deref( &self ) -> &Self::Target {
        &self.0
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn quote_simple() {
        let quoted = QuotedString::quote( "tralala" ).unwrap();
        assert_eq!( "\"tralala\"", &**quoted );
    }

    #[test]
    fn quote_some_chars() {
        let quoted = QuotedString::quote( "tr@al al\"a" ).unwrap();
        assert_eq!(  "\"tr@al\\ al\\\"a\"", &**quoted );
    }

    #[test]
    fn quote_ctl() {
        let res = QuotedString::quote("\x01");
        assert_eq!( false, res.is_ok() );
    }

    #[test]
    fn unquote_simple() {
        let quoted = QuotedString::parse( "\"simple\"".into() ).unwrap();
        assert_eq!( "simple", &*quoted.unquote() )
    }

    #[test]
    fn unquote() {
        let quoted = QuotedString::parse( r#""\ \\_\"<>""#.into() ).unwrap();
        assert_eq!( r#" \_"<>"#, &*quoted.unquote() )
    }

    #[test]
    fn end2end() {
        let samples = &[
            "abc",
            "ab cd",
            "a \" v@d",
            r#""\ ""#
        ];
        for sample in samples {
            assert_eq!( *sample, &*QuotedString::quote( sample ).unwrap().unquote() )
        }
    }
}


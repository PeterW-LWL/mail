use std::ops::Deref;

use ascii::AsciiString;

use error::*;
use grammar::MailType;
use grammar::is_quoted_string;
use codec::{ EncodeHeaderHandle, self};

use super::simple_item::SimpleItem;
use super::inner_item::{ InnerAscii, InnerUtf8 };

#[derive( Debug, Clone, Hash, Eq, PartialEq )]
pub struct QuotedString( SimpleItem );


impl QuotedString {

    pub fn write_into( handle: &mut EncodeHeaderHandle, input: &str ) -> Result<()> {
        //OPTIMIZE: do not unnecessarily allocate strings, but directly write to Encoder
        use self::SimpleItem::*;
        let quoted = QuotedString::quote( input )?;
        match *quoted {
            Ascii( ref inner ) => {
                handle.write_str( &*inner );
            },
            Utf8( ref inner ) => {
                handle.write_utf8( &*inner )?
            }
        }
        Ok( () )
    }

    pub fn parse( already_quoted: SimpleItem ) -> Result<Self> {
        if is_quoted_string( &*already_quoted, MailType::Internationalized ) {
            Ok( QuotedString( already_quoted ) )
        } else {
            bail!( "already quoted item is not actualy valid: {:?}", &*already_quoted );
        }
    }

    pub fn quote( input: &str ) -> Result<Self> {
        let (mt, res) = codec::quoted_string::quote(input)?;
        if mt == MailType::Ascii {
            let asciied = unsafe { AsciiString::from_ascii_unchecked( res ) };
            Ok( QuotedString( asciied.into() ) )
        } else {
            Ok( QuotedString( SimpleItem::from_utf8( res.into() ) ) )
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
        assert_eq!(  "\"tr@al al\\\"a\"", &**quoted );
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


use std::ops::Deref;

use error::*;
use ascii::AsciiString;
use grammar::{is_qtext, is_ws, is_vchar, MailType };

//FIXME prevent construction of invalide Quoted
#[derive( Debug, Clone, Hash, Eq, PartialEq )]
pub enum Quoted {
    //FIXME use Item/InnerAsciiItem after moving it out of components
    Ascii( AsciiString ),
    //FIXME use Item/InnerUtf8Item after movinf it out of components
    Utf8( String )
}

impl Quoted {
    pub fn into_string( self ) -> String {
        use self::Quoted::*;
        match self {
            Ascii(astring) => astring.into(),
            Utf8( string ) => string

        }
    }


}

impl Deref for Quoted {
    type Target = str;

    fn deref( &self ) -> &Self::Target {
        use self::Quoted::*;
        match *self {
            Ascii( ref astr) => (&**astr).as_str(),
            Utf8( ref ustr ) => &**ustr
        }
    }
}

//FIXME use SimpleItem once it has been moved out of components
pub fn quote( input: &str ) -> Result<Quoted> {
    let mut is_ascii = true;
    let mut out = String::new();
    out.push( '"' );
    for char in input.chars() {
        if is_ascii { is_ascii = (char as u32 & !0x7F_u32) == 0 }
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
    if is_ascii {
        Ok( Quoted::Ascii( unsafe { AsciiString::from_ascii_unchecked( out ) } ) )
    } else {
        Ok( Quoted::Utf8( out ) )
    }
}

//FIXME return SimpleItem once it has been moved out of components
/// # Panics
/// panics if the Quoted string was internally not valid
///
pub fn unquote( quoted: &Quoted) -> String {
    let quoted: &str = &**quoted;
    if quoted.len() < 2 { panic!( "invalide quoted string" ) }

    //FEATURE_TODO(bug_errors): use internal error (==BUG) sub error chain here
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

    out
}
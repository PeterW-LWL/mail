use std::ops::Deref;
use ascii::{ AsciiString, AsciiStr };

use super::input::Input;
use super::inner_item::{ InnerAsciiItem, InnerUtf8Item };

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
pub enum SimpleItem {
    /// specifies that the Item is valid Ascii, nothing more
    Ascii( InnerAsciiItem ),
    /// specifies that the Item is valid Utf8, nothing more
    Utf8( InnerUtf8Item )
}

impl SimpleItem {

    pub fn is_ascii( &self ) -> bool {
        use self::SimpleItem::*;
        match *self {
            Ascii( .. ) => true,
            Utf8( .. ) => false
        }
    }

    pub fn from_utf8( s: String ) -> Self {
        SimpleItem::Utf8( InnerUtf8Item::Owned( s ) )
    }


}

impl Deref for SimpleItem {
    type Target = str;

    fn deref( &self ) -> &str {
        use self::SimpleItem::*;
        match *self {
            Ascii( ref astr ) => astr.as_str(),
            Utf8( ref utf8 ) => &**utf8
        }
    }
}


impl Into<String> for SimpleItem {
    fn into(self) -> String {
        use self::SimpleItem::*;
        match self {
            Ascii( aitem ) => {
                let astring: AsciiString = aitem.into();
                astring.into()
            },
            Utf8( string ) => string.into()
        }
    }
}

impl<'a> From<&'a str> for SimpleItem {
    fn from( string: &'a str ) -> Self {
        Self::from( String::from( string ) )
    }
}

impl From<String> for SimpleItem {
    fn from( string: String ) -> Self {
        match AsciiString::from_ascii( string ) {
            Ok( astring ) => SimpleItem::Ascii( InnerAsciiItem::Owned( astring ) ),
            Err( err ) => SimpleItem::Utf8( InnerUtf8Item::Owned( err.into_source() ) )
        }
    }
}

impl From<AsciiString> for SimpleItem {
    fn from( astring: AsciiString ) -> Self {
        SimpleItem::Ascii( InnerAsciiItem::Owned( astring ) )
    }
}

impl From<Input> for SimpleItem {
    fn from(input: Input) -> Self {
        match input {
            Input( InnerUtf8Item::Owned( string ) ) => match AsciiString::from_ascii( string ) {
                Ok( ascii ) => SimpleItem::Ascii( InnerAsciiItem::Owned( ascii ) ),
                Err( err ) => SimpleItem::Utf8( InnerUtf8Item::Owned( err.into_source() ) )
            },
            Input( InnerUtf8Item::Shared( shared ) ) => {
                if AsciiStr::from_ascii( &*shared ).is_ok() {
                    SimpleItem::Ascii( InnerAsciiItem::Owned( unsafe {
                        AsciiString::from_ascii_unchecked( String::from( &*shared ) )
                    } ) )
                } else {
                    SimpleItem::Utf8( InnerUtf8Item::Shared( shared ) )
                }
            }
        }
    }
}

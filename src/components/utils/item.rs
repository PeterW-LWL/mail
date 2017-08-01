use serde;
use std::rc::Rc;
use std::ops::{ Deref };
use std::cmp::PartialEq;
use std::result::{ Result as StdResult };

use owning_ref::OwningRef;
use ascii::{ AsciiString, AsciiStr, FromAsciiError };

use codec::quote::Quoted;

//FEATURE_TODO(non_utf8_input): use (Vec<u8>, Encoding) instead of String in Input
//  but keep String in item, as there non utf8 input is not allowed

/// a Input is similar to Item a container data container used in different
/// context's with different restrictions, but different to an Item it
/// might contain characters which require encoding (e.g. encoded words)
/// to represent them
#[derive(Debug, Clone, Hash, Eq )]
pub enum Input {
    Owned(String),
    Shared(OwningRef<Rc<String>, str>)
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Item {
    /// A Item::Input, differs to Input as there can already be some restrictions on it,
    /// e.g. a Item::Input in a Word is meant to be _one_ (possible encoded) word
    Input(Input),

    /// A Item which is an encoded word
    EncodedWord(InnerAsciiItem),

    /// A quoted string
    QuotedString(Quoted),


    //FEATURE_TODO(non_utf8_input):
    // NonUtf8Input(...)
}


#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
pub enum SimpleItem {
    /// specifies that the Item is valid Ascii, nothing more
    Ascii( InnerAsciiItem ),
    /// specifies that the Item is valid Utf8, nothing more
    Utf8( InnerUtf8Item )
}

impl Input {

    pub fn into_shared( self ) -> Self {
        use self::Input::*;
        match self {
            Owned( value ) => Shared( OwningRef::new( Rc::new( value ) ).map( |rced| &**rced ) ),
            v @ Shared( .. ) => v
        }
    }

    pub fn into_simple_item( self ) -> SimpleItem {
        match self {
            Input::Owned( string ) => match AsciiString::from_ascii( string ) {
                Ok( ascii ) => SimpleItem::Ascii( InnerAsciiItem::Owned( ascii ) ),
                Err( err ) => SimpleItem::Utf8( InnerUtf8Item::Owned( err.into_source() ) )
            },
            Input::Shared( shared ) => {
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

    pub fn into_ascii_item( self ) -> StdResult<InnerAsciiItem, FromAsciiError<String>> {
        Ok( match self {
            Input::Owned( string )
                => InnerAsciiItem::Owned( AsciiString::from_ascii( string )? ),
            Input::Shared( shared )
                => InnerAsciiItem::Owned(
                    AsciiString::from_ascii( String::from( &*shared ) )? )
        } )
    }

    pub unsafe fn into_ascii_item_unchecked( self ) -> InnerAsciiItem {
        match self {
            Input::Owned( string )
                => InnerAsciiItem::Owned( AsciiString::from_ascii_unchecked( string ) ),
            Input::Shared( shared )
                => InnerAsciiItem::Owned(
                    AsciiString::from_ascii_unchecked( String::from( &*shared ) ) )
        }
    }

    pub fn into_utf8_item( self ) -> InnerUtf8Item {
        match self {
            Input::Owned( string ) => InnerUtf8Item::Owned( string ),
            Input::Shared( orwf ) => InnerUtf8Item::Shared( orwf )
        }
    }
}

impl PartialEq for Input {
    fn eq(&self, other: &Input) -> bool {
        let me: &str = &*self;
        let other: &str = &*other;
        me == other
    }
}

impl<'a> From<&'a str> for Input {
    fn from( s: &'a str ) -> Self {
        Input::Owned( s.into() )
    }
}
impl From<String> for Input {
    fn from( s: String ) -> Self {
        Input::Owned( s )
    }
}

impl Deref for Input {
    type Target = str;

    fn deref( &self ) -> &str {
        use self::Input::*;
        match *self {
            Owned( ref string ) => &*string,
            Shared( ref owning_ref ) => &*owning_ref
        }
    }
}


macro_rules! inner_impl {
    ($name:ident, $owned_form:ty, $borrowed_form:ty) => (

        /// a InnerItem is something potential appearing in Mail, e.g. an encoded word, an
        /// atom or a email address, but not some content which has to be represented
        /// as an encoded word, as such String is a suite representation,
        #[derive(Debug, Clone, Hash, Eq)]
        pub enum $name {
            Owned($owned_form),
            Shared(OwningRef<Rc<$owned_form>, $borrowed_form>)
        }

        impl $name {
            pub fn new<S: Into<$owned_form>>( data: S ) -> $name {
                $name::Owned( data.into() )
            }
        }

        impl<S> From<S> for $name where S: Into<$owned_form> {
            fn from( data: S ) -> Self {
                Self::new( data )
            }
        }

        impl Deref for $name {
            type Target = $borrowed_form;

            fn deref( &self ) -> &$borrowed_form{
                match *self {
                    $name::Owned( ref string ) => &*string,
                    $name::Shared( ref owning_ref ) => &*owning_ref
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: serde::Serializer
            {
                let borrowed: &$borrowed_form = &*self;
                let as_ref: &str = borrowed.as_ref();
                serializer.serialize_str( as_ref )
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &$name) -> bool {
                let me: &$borrowed_form = &*self;
                let other: &$borrowed_form = &*other;
                me == other
            }
        }

    )
}

inner_impl!{ InnerAsciiItem, AsciiString, AsciiStr }
inner_impl!{ InnerUtf8Item, String, str }
//inner_impl!{ InnerOtherItem, OtherString, OtherStr }


#[cfg(test)]
mod test {
    use std::str::FromStr;
    use super::*;

    #[test]
    fn input_eq() {
        let a = Input::Owned( "same".into() );
        let b = Input::Shared(
            OwningRef::new(
                Rc::new( String::from( "same" ) ) )
            .map(|v| &**v)
        );
        assert_eq!( a, b );
    }

    #[test]
    fn input_neq() {
        let a = Input::Owned( "not same".into() );
        let b = Input::Shared(
            OwningRef::new(
                Rc::new( String::from( "not at all same" ) ) )
                .map(|v| &**v)
        );
        assert_ne!( a, b );
    }

    #[test]
    fn inner_ascii_item_eq() {
        let a = InnerAsciiItem::Owned( AsciiString::from_str( "same" ).unwrap() );
        let b = InnerAsciiItem::Shared(
            OwningRef::new(
                Rc::new( AsciiString::from_str( "same" ).unwrap() ) )
                .map(|v| &**v)
        );
        assert_eq!( a, b );
    }

    #[test]
    fn inner_ascii_item_neq() {
        let a = InnerAsciiItem::Owned( AsciiString::from_str( "same" ).unwrap() );
        let b = InnerAsciiItem::Shared(
            OwningRef::new(
                Rc::new( AsciiString::from_str( "not same" ).unwrap() ) )
                .map(|v| &**v)
        );
        assert_ne!( a, b );
    }

    #[test]
    fn inner_utf8_item_eq() {
        let a = InnerUtf8Item::Owned( String::from( "same" ) );
        let b = InnerUtf8Item::Shared(
            OwningRef::new(
                Rc::new( String::from( "same" ) ) )
                .map(|v| &**v)
        );
        assert_eq!( a, b );
    }

    #[test]
    fn inner_utf8_item_neq() {
        let a = InnerUtf8Item::Owned( String::from( "same" ) );
        let b = InnerUtf8Item::Shared(
            OwningRef::new(
                Rc::new( String::from( "not same" ) ) )
                .map(|v| &**v)
        );
        assert_ne!( a, b );
    }

}
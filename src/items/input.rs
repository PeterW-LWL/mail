use std::result::{ Result as StdResult };

use ascii::{ AsciiString, FromAsciiError };

use super::inner_item::{ InnerUtf8Item, InnerAsciiItem };

/// a Input is similar to Item a container data container used in different
/// context's with different restrictions, but different to an Item it
/// might contain characters which require encoding (e.g. encoded words)
/// to represent them
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Input( pub InnerUtf8Item );


impl Input {

    pub fn into_shared( self ) -> Self {
        Input( self.0.into_shared() )
    }


    pub fn into_ascii_item( self ) -> StdResult<InnerAsciiItem, FromAsciiError<String>> {
        Ok( match self {
            Input( InnerUtf8Item::Owned( string ) ) =>
                InnerAsciiItem::Owned( AsciiString::from_ascii( string )? ),
            Input( InnerUtf8Item::Shared( shared ) ) =>
                InnerAsciiItem::Owned( AsciiString::from_ascii( String::from( &*shared ) )? )
        } )
    }

    pub unsafe fn into_ascii_item_unchecked( self ) -> InnerAsciiItem {
        match self {
            Input( InnerUtf8Item::Owned( string ) ) =>
                InnerAsciiItem::Owned( AsciiString::from_ascii_unchecked( string ) ),
            Input( InnerUtf8Item::Shared( shared ) ) =>
                InnerAsciiItem::Owned(
                    AsciiString::from_ascii_unchecked( String::from( &*shared ) ) )
        }
    }

    pub fn into_utf8_item( self ) -> InnerUtf8Item {
        self.0
    }
}

impl<'a> From<&'a str> for Input {
    fn from( s: &'a str ) -> Self {
        Input( InnerUtf8Item::Owned( s.into() ) )
    }
}
impl From<String> for Input {
    fn from( s: String ) -> Self {
        Input( InnerUtf8Item::Owned( s ) )
    }
}


deref0!( +mut Input => InnerUtf8Item );



#[cfg(test)]
mod test {
    use std::rc::Rc;
    use owning_ref::OwningRef;

    use super::*;

    #[test]
    fn input_eq() {
        let a = Input( InnerUtf8Item::Owned( "same".into() ) );
        let b = Input( InnerUtf8Item::Shared(
            OwningRef::new(
                Rc::new( String::from( "same" ) ) )
                .map(|v| &**v)
        ) );
        assert_eq!( a, b );
    }

    #[test]
    fn input_neq() {
        let a = Input( InnerUtf8Item::Owned( "not same".into() ) );
        let b = Input( InnerUtf8Item::Shared(
            OwningRef::new(
                Rc::new( String::from( "not at all same" ) ) )
                .map(|v| &**v)
        ) );
        assert_ne!( a, b );
    }



}
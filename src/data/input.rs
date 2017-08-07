use std::result::{ Result as StdResult };

use ascii::{ AsciiString, FromAsciiError };

use super::inner_item::{ InnerUtf8, InnerAscii };

/// a Input is similar to Item a container data container used in different
/// context's with different restrictions, but different to an Item it
/// might contain characters which require encoding (e.g. encoded words)
/// to represent them
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Input( pub InnerUtf8 );


impl Input {

    pub fn into_shared( self ) -> Self {
        Input( self.0.into_shared() )
    }


    pub fn into_ascii_item( self ) -> StdResult<InnerAscii, FromAsciiError<String>> {
        Ok( match self {
            Input( InnerUtf8::Owned( string ) ) =>
                InnerAscii::Owned( AsciiString::from_ascii( string )? ),
            Input( InnerUtf8::Shared( shared ) ) =>
                InnerAscii::Owned( AsciiString::from_ascii( String::from( &*shared ) )? )
        } )
    }

    pub unsafe fn into_ascii_item_unchecked( self ) -> InnerAscii {
        match self {
            Input( InnerUtf8::Owned( string ) ) =>
                InnerAscii::Owned( AsciiString::from_ascii_unchecked( string ) ),
            Input( InnerUtf8::Shared( shared ) ) =>
                InnerAscii::Owned(
                    AsciiString::from_ascii_unchecked( String::from( &*shared ) ) )
        }
    }

    pub fn into_utf8_item( self ) -> InnerUtf8 {
        self.0
    }
}

impl<'a> From<&'a str> for Input {
    fn from( s: &'a str ) -> Self {
        Input( InnerUtf8::Owned( s.into() ) )
    }
}
impl From<String> for Input {
    fn from( s: String ) -> Self {
        Input( InnerUtf8::Owned( s ) )
    }
}


deref0!( +mut Input => InnerUtf8 );



#[cfg(test)]
mod test {
    use std::rc::Rc;
    use owning_ref::OwningRef;

    use super::*;

    #[test]
    fn input_eq() {
        let a = Input( InnerUtf8::Owned( "same".into() ) );
        let b = Input( InnerUtf8::Shared(
            OwningRef::new(
                Rc::new( String::from( "same" ) ) )
                .map(|v| &**v)
        ) );
        assert_eq!( a, b );
    }

    #[test]
    fn input_neq() {
        let a = Input( InnerUtf8::Owned( "not same".into() ) );
        let b = Input( InnerUtf8::Shared(
            OwningRef::new(
                Rc::new( String::from( "not at all same" ) ) )
                .map(|v| &**v)
        ) );
        assert_ne!( a, b );
    }



}
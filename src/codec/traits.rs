use std::any::{ Any, TypeId };
use std::boxed::Box;
use std::result::{ Result as StdResult };
use std::fmt::Debug;

use ascii::{  AsciiStr, AsciiChar };

use error::*;
use grammar::MailType;

use super::EncodedWordEncoding;

pub trait EncodedWordWriter {
    fn write_char( &mut self, ch: AsciiChar );
    fn write_charset( &mut self );
    fn encoding( &self ) -> EncodedWordEncoding;
    fn write_ecw_seperator( &mut self );

    /// Returns the maximal length of the paylod/encoded data
    ///
    /// Any number of calls to methods on in trait in any way
    /// should never be able to change the returned value.
    /// Only changing e.g. the charset or encoding should be
    /// able to change what `max_paylod_len` returns.
    fn max_payload_len( &self ) -> usize;

    fn write_ecw_start( &mut self ) {
        self.write_char( AsciiChar::Equal );
        self.write_char( AsciiChar::Question );
        self.write_charset();
        self.write_char( AsciiChar::Question );
        let acronym = self.encoding().acronym();
        self.write_str( acronym );
        self.write_char( AsciiChar::Question );
    }

    fn write_ecw_end( &mut self ) {
        self.write_char( AsciiChar::Question );
        self.write_char( AsciiChar::Equal );
    }


    fn start_next_encoded_word( &mut self )  {
        self.write_ecw_end();
        self.write_ecw_seperator();
        self.write_ecw_start();
    }

    fn write_str( &mut self, str: &AsciiStr ) {
        for char in str {
            self.write_char(*char)
        }
    }
}

pub trait MailEncoder: 'static {
    fn mail_type( &self ) -> MailType;

    fn write_new_line( &mut self );
    fn write_fws( &mut self );
    fn note_optional_fws(&mut self );

    fn write_char( &mut self, char: AsciiChar );
    fn write_str( &mut self, str: &AsciiStr );

    //FIXME default impl
    fn try_write_utf8( &mut self, str: &str ) -> Result<()>;
    fn try_write_atext( &mut self, str: &str ) -> Result<()>;
    //fn write_encoded_word( &mut self, data: &str, ctx: EncodedWordContext );

    /// writes a string to the encoder without checking if it is compatible
    /// with the mail type, if not used correctly this can write Utf8 to
    /// an Ascii Mail, which is incorrect but has to be safe wrt. rust's safety.
    fn write_str_unchecked( &mut self, str: &str);


    fn current_line_byte_length(&self ) -> usize;

    //could also be called write_data_unchecked
    fn write_body( &mut self, body: &[u8]);
}

pub trait MailEncodable<E: MailEncoder>: Any+Debug {
    fn encode( &self, encoder:  &mut E ) -> Result<()>;

    #[doc(hidden)]
    fn type_id( &self ) -> TypeId {
        TypeId::of::<Self>()
    }
}


// cant use some of the macros from cares.io which
// do this for you as we need support for an generic
// variable and where clause
impl<E> MailEncodable<E>
    where E: MailEncoder
{

    #[inline(always)]
    pub fn is<T: MailEncodable<E>>(&self) -> bool {
        self.type_id() == TypeId::of::<T>()
    }


    #[inline]
    pub fn downcast_ref<T: MailEncodable<E>>(&self) -> Option<&T> {
        if self.is::<T>() {
            Some( unsafe { &*( self as *const MailEncodable<E> as *const T) } )
        } else {
            None
        }
    }

    #[inline]
    pub fn downcast_mut<T: MailEncodable<E>>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            Some( unsafe { &mut *( self as *mut MailEncodable<E> as *mut T) } )
        } else {
            None
        }
    }
}


pub trait MailEncodableBoxExt<E: MailEncoder>: Sized {
    fn downcast<T: MailEncodable<E>>(self) -> StdResult<Box<T>, Self>;
}

impl<E> MailEncodableBoxExt<E> for Box<MailEncodable<E>>
    where E: MailEncoder
{
    fn downcast<T: MailEncodable<E>>(self) -> StdResult<Box<T>, Self> {
        if <MailEncodable<E>>::is::<T>(&*self) {
            let ptr: *mut MailEncodable<E> = Box::into_raw(self);
            Ok( unsafe { Box::from_raw(ptr as *mut T) } )
        } else {
            Err( self )
        }
    }
}

impl<E> MailEncodableBoxExt<E> for Box<MailEncodable<E>+Send>
    where E: MailEncoder
{
    fn downcast<T: MailEncodable<E>>(self) -> StdResult<Box<T>, Self> {
        if <MailEncodable<E>>::is::<T>(&*self) {
            let ptr: *mut MailEncodable<E> = Box::into_raw(self);
            Ok( unsafe { Box::from_raw(ptr as *mut T) } )
        } else {
            Err( self )
        }
    }
}

#[cfg(test)]
mod test {
    use error::*;
    use codec::{
        MailEncoder, MailEncodable,
        MailEncodableBoxExt, MailEncoderImpl
    };

    #[derive(Default, PartialEq, Debug)]
    struct TestType(&'static str);

    impl<E> MailEncodable<E> for TestType
        where E: MailEncoder
    {
        fn encode( &self, encoder:  &mut E ) -> Result<()> {
            encoder.write_str_unchecked(self.0);
            Ok(())
        }
    }

    #[derive(Default, PartialEq, Debug)]
    struct AnotherType(&'static str);

    impl<E> MailEncodable<E> for AnotherType
        where E: MailEncoder
    {
        fn encode( &self, encoder:  &mut E ) -> Result<()> {
            encoder.write_str_unchecked(self.0);
            Ok(())
        }
    }



    #[test]
    fn is() {
        let tt = TestType::default();
        let erased: &MailEncodable<MailEncoderImpl> = &tt;
        assert_eq!( true, erased.is::<TestType>() );
        assert_eq!( false, erased.is::<AnotherType>());
    }

    #[test]
    fn downcast_ref() {
        let tt = TestType::default();
        let erased: &MailEncodable<MailEncoderImpl> = &tt;
        let res: Option<&TestType> = erased.downcast_ref::<TestType>();
        assert_eq!( Some(&tt), res );
        assert_eq!( None, erased.downcast_ref::<AnotherType>() );
    }

    #[test]
    fn downcast_mut() {
        let mut tt_nr2 = TestType::default();
        let mut tt = TestType::default();
        let erased: &mut MailEncodable<MailEncoderImpl> = &mut tt;
        {
            let res: Option<&mut TestType> = erased.downcast_mut::<TestType>();
            assert_eq!( Some(&mut tt_nr2), res );
        }
        assert_eq!( None, erased.downcast_mut::<AnotherType>() );
    }

    #[test]
    fn downcast() {
        let tt = Box::new( TestType::default() );
        let erased: Box<MailEncodable<MailEncoderImpl>> = tt;
        let erased = assert_err!(erased.downcast::<AnotherType>());
        let _: Box<TestType> = assert_ok!(erased.downcast::<TestType>());
    }

}
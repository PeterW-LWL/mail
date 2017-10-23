use error::Result;



mod quoted;
pub  use self::quoted::*;

pub mod encoded_word;
pub use self::encoded_word::*;

use utils::HeaderTryFrom;

//FEATURE_TODO(non_utf8_input): use (Vec<u8>, Encoding) instead of String in Input
//  but keep String in item, as there non utf8 input is not allowed

//TODO remove FromInput Trait replace all usages with HeaderTryFrom
pub trait FromInput: Sized {
    fn from_input<I: Into<Input>>( input: I ) -> Result<Self>;
}


impl<'a, T> HeaderTryFrom<&'a str> for T
    where T: FromInput
{
    fn try_from(val: &'a str) -> Result<Self> {
        T::from_input( val )
    }
}
impl<T> HeaderTryFrom<String> for T
    where T: FromInput
{
    fn try_from(val: String) -> Result<Self> {
        T::from_input( val )
    }
}



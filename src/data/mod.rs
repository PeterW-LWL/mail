use error::Result;

mod inner_item;
pub use self::inner_item::*;

mod input;
pub use self::input::*;

mod simple_item;
pub use self::simple_item::*;

mod quoted;
pub  use self::quoted::*;

pub mod encoded_word;
pub use self::encoded_word::*;

//FEATURE_TODO(non_utf8_input): use (Vec<u8>, Encoding) instead of String in Input
//  but keep String in item, as there non utf8 input is not allowed

pub trait FromInput: Sized {
    fn from_input<I: Into<Input>>( input: I ) -> Result<Self>;
}




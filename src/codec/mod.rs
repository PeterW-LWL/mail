use ascii::{ AsciiStr, AsciiChar};

pub mod transfer_encoding;
pub mod quoted_printable;
pub mod base64;
pub mod idna;
pub mod mime;
pub mod quoted_string;

#[cfg(test)]
#[macro_use]
pub mod test_utils;

mod traits;
pub use self::traits::*;

mod encoder_impl;
pub use self::encoder_impl::*;


mod writer_impl;
pub use self::writer_impl::*;


#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum EncodedWordEncoding {
    Base64, QuotedPrintable
}

impl EncodedWordEncoding {

    /// returns the acronym for the given encoding
    /// used in a encoded word
    pub fn acronym(&self) -> &'static AsciiStr {
        use self::EncodedWordEncoding::*;

        static BASE64: &[AsciiChar] = &[ AsciiChar::B ];
        static QUOTED: &[AsciiChar] = &[ AsciiChar::Q ];

        match *self {
            Base64 => BASE64.into(),
            QuotedPrintable => QUOTED.into()
        }
    }

    /// encodes a given utf8 string
    ///
    /// either `self::quoted_printable::encoded_word_encode`
    /// or `self::base64::encoded_word_encode_utf8` is used
    /// depending on which value `self` is.
    ///
    /// As both algorithm need to know about code point boundaries
    /// only encoding utf8 is supported for now
    ///
    pub fn encode<R, O>(&self, input: R, out: &mut O)
        where R: AsRef<str>, O: EncodedWordWriter
    {
        use self::EncodedWordEncoding::*;
        let input: &str = input.as_ref();
        match *self {
            Base64 => {
                base64::encoded_word_encode(input, out)
            },
            QuotedPrintable => {
                quoted_printable::encoded_word_encode_utf8(input, out)
            }
        }
    }
}
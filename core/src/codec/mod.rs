use soft_ascii_string::SoftAsciiStr;

pub mod transfer_encoding;
pub mod quoted_printable;
pub mod base64;
pub mod idna;
pub mod mime;


mod traits;
pub use self::traits::*;

#[cfg_attr(test, macro_use)]
mod encoder;
pub use self::encoder::*;

mod writer_impl;
pub use self::writer_impl::*;


#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum EncodedWordEncoding {
    Base64, QuotedPrintable
}

impl EncodedWordEncoding {

    /// returns the acronym for the given encoding
    /// used in a encoded word
    pub fn acronym(&self) -> &'static SoftAsciiStr {
        use self::EncodedWordEncoding::*;
        match *self {
            Base64 => SoftAsciiStr::from_str_unchecked("B"),
            QuotedPrintable => SoftAsciiStr::from_str_unchecked("Q")
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
use std::ops::Deref;

use soft_ascii_string::{ SoftAsciiString, SoftAsciiStr };

use error::*;
use codec::{EncodableInHeader, EncodeHandle};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum TransferEncoding {
    _7Bit,
    _8Bit,
    Binary,
    QuotedPrintable,
    Base64,
    // should be only ietf-token (i.e. tokens standarized through an RFC and registered with IANA)
    // but we don't check this so it's other and not ietf token
    //FIXME not sure if the limitations are to tight (with Token)
    //FIXME allow puting XTokens into OtherToken when generating?
    Other( Token ),
}

impl TransferEncoding {
    pub fn name( &self ) -> &SoftAsciiStr {
        use self::TransferEncoding::*;
        match *self {
            _7Bit => SoftAsciiStr::from_str_unchecked("7bit"),
            _8Bit => SoftAsciiStr::from_str_unchecked("8bit"),
            Binary =>  SoftAsciiStr::from_str_unchecked("binary"),
            QuotedPrintable => SoftAsciiStr::from_str_unchecked("quoted-printable"),
            Base64 =>  SoftAsciiStr::from_str_unchecked("base64"),
            Other( ref token ) => &*token
        }
    }
}

impl EncodableInHeader for  TransferEncoding {

    fn encode(&self, handle: &mut EncodeHandle) -> Result<()> {
        handle.write_str( self.name() )?;
        Ok( () )
    }
}



#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Token( SoftAsciiString );

impl Token {

    //TODO limit chars valid for token (no space, no special chars like {([" ... )
    // and implement some form of constructor

    pub fn is_x_token( &self ) -> bool {
        let bytes = self.as_bytes();
        bytes[1] == b'-' && ( bytes[0] == b'X' || bytes[0] == b'x' )
    }
}

impl  Deref for Token {
    type Target = SoftAsciiStr;
    fn deref( &self ) -> &SoftAsciiStr {
        &*self.0
    }
}


#[cfg(test)]
mod test {
    use super::TransferEncoding;

    ec_test! {_7bit, {
        TransferEncoding::_7Bit
    } => ascii => [
        Text "7bit"
    ]}

    ec_test! {_8bit, {
        TransferEncoding::_8Bit
    } => ascii => [
        Text "8bit"
    ]}

    ec_test!{binary, {
        TransferEncoding::Binary
    } => ascii => [
        Text "binary"
    ]}

    ec_test!{base64, {
        TransferEncoding::Base64
    } => ascii => [
        Text "base64"
    ]}

    ec_test!{quoted_printable, {
        TransferEncoding::QuotedPrintable
    } => ascii => [
        Text "quoted-printable"
    ]}
}
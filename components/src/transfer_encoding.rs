use std::ops::Deref;

use soft_ascii_string::{ SoftAsciiString, SoftAsciiStr };

use core::error::*;
use core::codec::{EncodableInHeader, EncodeHandle};
use core::codec::transfer_encoding::TransferEncoding;

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
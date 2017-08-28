use ascii::{ AsciiString, AsciiStr };

use std::ops::Deref;
use error::*;
use codec::{ MailEncoder, MailEncodable };

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
    pub fn name( &self ) -> &AsciiStr {
        use self::TransferEncoding::*;
        match *self {
            _7Bit => ascii_str! { _7 b i t },
            _8Bit => ascii_str! { _8 b i t },
            Binary =>  ascii_str! { b i n a r y },
            QuotedPrintable =>  ascii_str! { q u o t e d Minus p r i n t a b l e },
            Base64 =>  ascii_str! { b a s e _6 _4 },
            Other( ref token ) => &*token
        }
    }
}

impl<E> MailEncodable<E> for TransferEncoding where E: MailEncoder {

    fn encode(&self, encoder: &mut E) -> Result<()> {
        encoder.write_str( self.name() );
        Ok( () )
    }
}



#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Token( AsciiString );

impl Token {

    //TODO limit chars valid for token (no space, no special chars like {([" ... )
    // and implement some form of constructor

    pub fn is_x_token( &self ) -> bool {
        let bytes = self.as_bytes();
        bytes[1] == b'-' && ( bytes[0] == b'X' || bytes[0] == b'x' )
    }
}

impl  Deref for Token {
    type Target = AsciiStr;
    fn deref( &self ) -> &AsciiStr {
        &*self.0
    }
}


#[cfg(test)]
mod test {
    use super::TransferEncoding;
    use codec::test_utils::*;

    ec_test! {_7bit, {
        Some( TransferEncoding::_7Bit )
    } => ascii => [
        LinePart( "7bit" )
    ]}

    ec_test! {_8bit, {
        Some( TransferEncoding::_8Bit )
    } => ascii => [
        LinePart( "8bit" )
    ]}

    ec_test!{binary, {
        Some( TransferEncoding::Binary )
    } => ascii => [
        LinePart( "binary" )
    ]}

    ec_test!{base64, {
        Some( TransferEncoding::Base64 )
    } => ascii => [
        LinePart( "base64" )
    ]}

    ec_test!{quoted_printable, {
        Some( TransferEncoding::QuotedPrintable )
    } => ascii => [
        LinePart( "quoted-printable" )
    ]}
}
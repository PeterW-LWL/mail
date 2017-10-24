//! mail-codec-core does not ship with any predefined headers and components
//! except `RawUnstructured`, `TransferEncoding` and `DateTime`

use soft_ascii_string::SoftAsciiStr;

use error::*;
use grammar::is_vchar;
use utils::{DateTime, HeaderTryFrom, HeaderTryInto};
use data::Input;
use codec::{EncodeHandle, EncodableInHeader};
use codec::transfer_encoding::TransferEncoding;

/// A unstructured header field implementation which validates the given input
/// but does not encode any utf8 even if it would have been necessary (it will
/// error in that case) nor does it support breaking longer lines in multiple
/// ones (no FWS marked for the encoder)
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct RawUnstructured {
    text: Input
}

impl RawUnstructured {
    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }
}

impl<T> From<T> for RawUnstructured
    where Input: From<T>
{
    fn from(val: T) -> Self {
        RawUnstructured { text: val.into() }
    }
}

impl<T> HeaderTryFrom<T> for RawUnstructured
    where T: HeaderTryInto<Input>
{
    fn try_from(val: T) -> Result<Self> {
        let input: Input = val.try_into()?;
        Ok( input.into() )
    }
}

impl Into<Input> for RawUnstructured {
    fn into(self) -> Input {
        self.text
    }
}

impl Into<String> for RawUnstructured {
    fn into(self) -> String {
        self.text.into()
    }
}

impl AsRef<str> for RawUnstructured {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl EncodableInHeader for RawUnstructured {
    fn encode(&self, handle: &mut EncodeHandle) -> Result<()> {
        let mail_type = handle.mail_type();

        if !self.text.chars().all(|ch| is_vchar(ch, mail_type)) {
            bail!("encoding error invalid content for raw unstructured: {:?} (mt: {:?})",
                self.text.as_str(),
                mail_type
            )
        }

        if handle.mail_type().is_internationalized() {
            handle.write_utf8(self.text.as_str())
        } else {
            handle.write_str(SoftAsciiStr::from_str_unchecked(self.text.as_str()))
        }
    }
}


// we reuse the TransferEncoding as component
impl EncodableInHeader for  TransferEncoding {

    fn encode(&self, handle: &mut EncodeHandle) -> Result<()> {
        handle.write_str( self.repr() )?;
        Ok( () )
    }
}

// we implement EncodableInHeader on DateTime, too
// it just makes thinks simpler
impl EncodableInHeader for DateTime {

    fn encode(&self, handle: &mut EncodeHandle) -> Result<()> {
        let as_str = self.to_rfc2822();
        let ascii = SoftAsciiStr::from_str_unchecked( &*as_str );
        handle.write_str( ascii )?;
        Ok( () )
    }
}



#[cfg(test)]
mod test {
    use super::{TransferEncoding, DateTime};

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

    ec_test!{ date_time, {
        DateTime::test_time( 45 )
    } => ascii => [
        Text "Tue,  6 Aug 2013 04:11:45 +0000"
    ]}
}


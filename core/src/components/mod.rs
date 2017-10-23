//! mail-codec-core does not ship with any predefined headers and components
//! except `RawUnstructured`.

use soft_ascii_string::SoftAsciiStr;

use error::*;
use grammar::is_vchar;
use data::Input;
use codec::{EncodeHandle, EncodableInHeader};

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
use soft_ascii_string::SoftAsciiChar;

use core::error::*;
use core::utils::{HeaderTryFrom, HeaderTryInto};
use core::codec::{EncodableInHeader, EncodeHandle};
use super::Email;


#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Path(pub Option<Email>);

impl HeaderTryFrom<Option<Email>> for Path {
    fn try_from(opt_mail: Option<Email>) -> Result<Self> {
        Ok( Path( opt_mail ) )
    }
}

impl<T> HeaderTryFrom<T> for Path
    where T: HeaderTryInto<Email>
{
    fn try_from(opt_mail: T) -> Result<Self> {
        Ok( Path( Some( opt_mail.try_into()? ) ) )
    }
}

impl EncodableInHeader for  Path {

    fn encode(&self, handle: &mut EncodeHandle) -> Result<()> {
        handle.mark_fws_pos();
        handle.write_char(SoftAsciiChar::from_char_unchecked('<'))?;
        if let Some( mail ) = self.0.as_ref() {
            mail.encode( handle )?;
        }
        handle.write_char(SoftAsciiChar::from_char_unchecked('>'))?;
        handle.mark_fws_pos();
        Ok( () )
    }
}
//NOTE for parsing we have to make sure to _require_ '<>' around the email

#[cfg(test)]
mod test {
    use super::*;
    use core::data::FromInput;

    ec_test!{empty_path, {
        Path( None )
    } => ascii => [
        MarkFWS,
        Text "<>",
        MarkFWS
    ]}

    ec_test!{simple_path, {
        Path( Some( Email::from_input( "abc@de.fg" )? ) )
    } => ascii => [
        MarkFWS,
        Text "<",
        MarkFWS,
        Text "abc",
        MarkFWS,
        Text "@",
        MarkFWS,
        Text "de.fg",
        MarkFWS,
        Text ">",
        MarkFWS
    ]}
}
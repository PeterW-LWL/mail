use ascii::AsciiChar;

use error::*;
use utils::{HeaderTryFrom, HeaderTryInto};
use codec::{EncodableInHeader, EncodeHeaderHandle};
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

    fn encode(&self, handle: &mut EncodeHeaderHandle) -> Result<()> {
        handle.mark_fws_pos();
        handle.write_char( AsciiChar::LessThan );
        if let Some( mail ) = self.0.as_ref() {
            mail.encode( handle )?;
        }
        handle.write_char( AsciiChar::GreaterThan );
        handle.mark_fws_pos();
        Ok( () )
    }
}
//NOTE for parsing we have to make sure to _require_ '<>' around the email

#[cfg(test)]
mod test {
    use super::*;
    use data::FromInput;

    ec_test!{empty_path, {
        Path( None )
    } => ascii => [
        MarkFWS,
        NowChar,
        Text "<",
        NowChar,
        Text ">",
        MarkFWS
    ]}

    ec_test!{simple_path, {
        Path( Some( Email::from_input( "abc@de.fg" )? ) )
    } => ascii => [
        MarkFWS,
        NowChar,
        Text "<",
        MarkFWS,
        NowStr,
        Text "abc",
        MarkFWS,
        NowChar,
        Text "@",
        MarkFWS,
        NowStr,
        Text "de.fg",
        MarkFWS,
        NowChar,
        Text ">",
        MarkFWS
    ]}
}
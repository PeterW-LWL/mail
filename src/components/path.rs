use ascii::AsciiChar;

use error::*;
use codec::{ MailEncoder, MailEncodable };
use super::Email;


#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Path(pub Option<Email>);


impl MailEncodable for Path {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()>
        where E: MailEncoder
    {
        encoder.note_optional_fws();
        encoder.write_char( AsciiChar::LessThan );
        if let Some( mail ) = self.0.as_ref() {
            mail.encode( encoder )?;
        }
        encoder.write_char( AsciiChar::GreaterThan );
        encoder.note_optional_fws();
        Ok( () )
    }
}
//NOTE for parsing we have to make sure to _require_ '<>' around the email
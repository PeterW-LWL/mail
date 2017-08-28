

use error::*;
use ascii::AsciiChar;
use codec::{MailEncoder, MailEncodable };

use utils::Vec1;
use super::Mailbox;


pub struct OptMailboxList( pub Vec<Mailbox> );
pub struct MailboxList( pub Vec1<Mailbox> );

impl MailboxList {
    pub fn from_single( m: Mailbox ) -> Self {
        MailboxList( Vec1::new( m ) )
    }
}


impl<E> MailEncodable<E> for OptMailboxList where E: MailEncoder {

    fn encode(&self, encoder: &mut E) -> Result<()> {
       encode_list( self.0.iter(), encoder )
    }
}

impl<E> MailEncodable<E> for MailboxList where E: MailEncoder {

    fn encode(&self, encoder: &mut E) -> Result<()> {
        encode_list( self.0.iter(), encoder )
    }
}

fn encode_list<'a, E, I>( list_iter: I, encoder: &mut E ) -> Result<()>
    where E: MailEncoder,
          I: Iterator<Item=&'a Mailbox>
{
    sep_for!{ mailbox in list_iter;
        sep {
            encoder.write_char( AsciiChar::Comma );
            encoder.write_fws();
        };
        mailbox.encode( encoder )?;
    }
    Ok( () )
}

deref0!{ +mut OptMailboxList => Vec<Mailbox> }
deref0!{ +mut MailboxList => Vec<Mailbox> }

#[cfg(test)]
mod test {
    use data::FromInput;
    use components::{ Mailbox, Email, Phrase };
    use codec::test_utils::*;
    use super::*;


    ec_test! { empty_list, {
        Some( OptMailboxList( Vec::new() ) )
    } => ascii => [

    ]}

    ec_test! { single, {
        Some( MailboxList( vec1![
            Mailbox {
                display_name: Some( Phrase::from_input( "hy ho" ).unwrap() ),
                email: Email::from_input( "ran@dom" ).unwrap()
            },
        ] ) )
    } => ascii => [
        LinePart( "hy" ),
        FWS,
        LinePart( "ho" ),
        FWS,
        LinePart( "<" ),
        OptFWS,
        LinePart( "ran" ),
        OptFWS,
        LinePart( "@" ),
        OptFWS,
        LinePart( "dom" ),
        OptFWS,
        LinePart( ">")
    ]}

    ec_test! { multiple, {
         Some( MailboxList( vec1![
            Mailbox {
                display_name: Some( Phrase::from_input( "hy ho" ).unwrap() ),
                email: Email::from_input( "nar@mod" ).unwrap()
            },
            Mailbox {
                display_name: None,
                email: Email::from_input( "ran@dom" ).unwrap()
            }
        ] ) )
    } => ascii => [
        LinePart( "hy" ),
        FWS,
        LinePart( "ho" ),
        FWS,
        LinePart( "<" ),
        OptFWS,
        LinePart( "nar" ),
        OptFWS,
        LinePart( "@" ),
        OptFWS,
        LinePart( "mod" ),
        OptFWS,
        LinePart( ">,"),
        FWS,
        LinePart( "<" ),
        OptFWS,
        LinePart( "ran" ),
        OptFWS,
        LinePart( "@" ),
        OptFWS,
        LinePart( "dom" ),
        OptFWS,
        LinePart( ">")


    ]}
}
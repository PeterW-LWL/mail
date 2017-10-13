use ascii::AsciiChar;

use error::*;
use codec::{EncodableInHeader, EncodeHandle};
use super::word::{ Word, do_encode_word };
use super::{ Email, Domain };


#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ReceivedToken {
    Word( Word ),
    Address( Email ),
    Domain( Domain )
}

impl EncodableInHeader for  ReceivedToken {

    fn encode(&self, handle: &mut EncodeHandle) -> Result<()> {
        use self::ReceivedToken::*;
        match *self {
            Word( ref word ) => {
                do_encode_word( word, handle, None )?;
            },
            Address( ref addr ) => {
                // we do not need to use <..> , but I think it's better and it is definitely
                // not wrong
                handle.write_char( AsciiChar::LessThan )?;
                addr.encode( handle )?;
                handle.write_char( AsciiChar::GreaterThan )?;
            },
            Domain( ref domain ) => {
                domain.encode( handle )?;
            }
        }
        Ok( () )
    }
}

#[cfg(test)]
mod test {
    use grammar::MailType;
    use data::FromInput;
    use codec::{Encoder, VecBodyBuf};
    use super::*;

    ec_test!{ a_domain, {
        Domain::from_input( "random.mailnot" )?
    } => ascii => [
        MarkFWS,
        Text "random.mailnot",
        MarkFWS
    ]}

    ec_test!{ a_address, {
        let email = Email::from_input( "modnar@random.mailnot")?;
        ReceivedToken::Address( email )
    } => ascii => [
        Text "<",
        MarkFWS,
        Text "modnar",
        MarkFWS,
        Text "@",
        MarkFWS,
        Text "random.mailnot",
        MarkFWS,
        Text ">"
    ]}

    ec_test!{ a_word, {
        let word = Word::from_input( "simple" )?;
        ReceivedToken::Word( word )
    } => ascii => [
        Text "simple"
    ]}

    ec_test!{ a_quoted_word, {
        let word = Word::from_input( "sim ple" )?;
        ReceivedToken::Word( word )
    } => ascii => [
        Text r#""sim ple""#
    ]}


    #[test]
    fn no_encoded_word() {
        let mut encoder = Encoder::<VecBodyBuf>::new( MailType::Ascii );
        let mut handle = encoder.encode_handle();
        let input = ReceivedToken::Word( Word::from_input( "↓right" ).unwrap() );
        assert_err!(input.encode( &mut handle ));
        handle.undo_header();
    }
}
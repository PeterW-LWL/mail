use ascii::AsciiChar;

use error::*;
use codec::{ MailEncoder, MailEncodable };
use super::word::{ Word, do_encode_word };
use super::{ Email, Domain };


#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ReceivedToken {
    Word( Word ),
    Address( Email ),
    Domain( Domain )
}

impl MailEncodable for ReceivedToken {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()>
        where E: MailEncoder
    {
        use self::ReceivedToken::*;
        match *self {
            Word( ref word ) => {
                do_encode_word( word, encoder, None )?;
            },
            Address( ref addr ) => {
                // we do not need to use <..> , but I think it's better and it is definitely
                // not wrong
                encoder.write_char( AsciiChar::LessThan );
                addr.encode( encoder )?;
                encoder.write_char( AsciiChar::GreaterThan );
            },
            Domain( ref domain ) => {
                domain.encode( encoder )?;
            }
        }
        Ok( () )
    }
}

#[cfg(test)]
mod test {
    use grammar::MailType;
    use data::FromInput;
    use codec::test_utils::*;
    use super::*;

    ec_test!{ a_domain, {
        Domain::from_input( "random.mailnot" )
    } => ascii => [
        OptFWS,
        LinePart( "random.mailnot" ),
        OptFWS
    ]}

    ec_test!{ a_address, {
        Email::from_input( "modnar@random.mailnot").map( |mail| {
            ReceivedToken::Address( mail )
        })
    } => ascii => [
        LinePart( "<" ),
        OptFWS,
        LinePart( "modnar" ),
        OptFWS,
        LinePart( "@" ),
        OptFWS,
        LinePart( "random.mailnot" ),
        OptFWS,
        LinePart( ">" )
    ]}

    ec_test!{ a_word, {
        Word::from_input( "simple" ).map( |word| {
            ReceivedToken::Word( word )
        })
    } => ascii => [
        LinePart( "simple" )
    ]}

    ec_test!{ a_quoted_word, {
        Word::from_input( "sim ple" ).map( |word|  {
            ReceivedToken::Word( word )
        } )
    } => ascii => [
        LinePart( r#""sim\ ple""# )
    ]}


    #[test]
    fn no_encoded_word() {
        use codec::MailEncodable;
        use codec::test_utils::TestMailEncoder;

        let mut ec = TestMailEncoder::new( MailType::Ascii );
        let input = ReceivedToken::Word( Word::from_input( "↓right" ).unwrap() );
        let res = input.encode( &mut ec );
        assert_eq!( false, res.is_ok() );
    }
}
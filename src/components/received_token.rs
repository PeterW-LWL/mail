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


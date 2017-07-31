use ascii::AsciiChar;

use error::*;
use codec::{ MailEncoder, MailEncodable };
use super::word::{ Word, do_encode_word };
use super::utils::item::Item;
use super::{ Email, Domain, CFWS };


#[derive( Debug, Clone, Eq, PartialEq, Hash )]
pub struct ReceivedTokenWord( Word );

impl ReceivedTokenWord {
    pub fn new( item: Item ) -> Result<Self> {
        Ok( ReceivedTokenWord( Word::new( item, false )? ) )
    }

    pub fn from_parts(
        left_padding: Option<CFWS>,
        item: Item,
        right_padding: Option<CFWS>,
    ) -> Result<Self> {
        Ok( ReceivedTokenWord( Word::from_parts( left_padding, item, right_padding, false )? ) )
    }

}

deref0!{ +mut ReceivedTokenWord => Word }

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ReceivedToken {
    Word( ReceivedTokenWord ),
    Address( Email ),
    Domain( Domain )
}

impl MailEncodable for ReceivedToken {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()> where E: MailEncoder {
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


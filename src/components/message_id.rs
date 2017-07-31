use ascii::AsciiChar;

use nom::IResult;

use error::*;
use types::Vec1;
use codec::{ MailEncoder, MailEncodable };

use super::utils::item::{ Input, SimpleItem };

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
pub struct MessageID {
    message_id: SimpleItem
}

impl MessageID {
    pub fn from_input( input: Input ) ->  Result<Self> {
        use self::parser_parts::{ id_left, id_right };

        match do_parse!( &*input,
            id_left >>
            char!( '@' ) >>
            id_right >>
            (())
        ) {
            IResult::Done( .. ) => {},
            _ => bail!( "invalid message id: {}", &*input )
        }

        Ok( MessageID { message_id: input.into_simple_item() } )
    }
}

impl MailEncodable for MessageID {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()> where E: MailEncoder {
        encoder.write_char( AsciiChar::LessThan );
        match self.message_id {
            SimpleItem::Ascii( ref ascii ) => encoder.write_str( ascii ),
            SimpleItem::Utf8( ref utf8 ) => encoder.try_write_utf8( utf8 )?
        }
        encoder.write_char( AsciiChar::GreaterThan );
        Ok( () )
    }
}

mod parser_parts {
    use nom::IResult;
    use char_validators::{ is_atext, is_dtext, MailType };

    pub fn id_left( input: &str ) -> IResult<&str, &str> {
        dot_atom_text( input )
    }
    pub fn id_right( input: &str ) -> IResult<&str, &str> {
        complete!(
            input,
            alt!(
                no_fold_literal |
                dot_atom_text
            )
        )
    }

    fn no_fold_literal( input: &str ) -> IResult<&str, &str> {
        recognize!( input,
            tuple!(
                char!( '[' ),
                take_while!( call!( is_dtext, MailType::Internationalized ) ),
                char!( ']' )
            )
        )
    }

    fn dot_atom_text(input: &str) -> IResult<&str, &str> {
        recognize!( input, complete!( tuple!(
            take_while1!( call!( is_atext, MailType::Internationalized ) ),
            many0!(tuple!(
                char!( '.' ),
                take_while!( call!( is_atext, MailType::Internationalized ) )
            ))
        ) ) )
    }
}

pub struct MessageIDList( pub Vec1<MessageID> );

deref0!{ +mut MessageIDList => Vec1<MessageID> }

impl MailEncodable for MessageIDList {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()> where E: MailEncoder {
        for msg_id in self.iter() {
            msg_id.encode( encoder )?;
        }
        Ok( () )
    }
}

//NOTE for parsing mails we have to make sure to _require_ '<>' around the email
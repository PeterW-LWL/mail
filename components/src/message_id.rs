
use nom::IResult;

use soft_ascii_string::SoftAsciiChar;
use vec1::Vec1;

use core::error::*;
use core::codec::{EncodableInHeader, EncodeHandle};
use core::data::{ FromInput, Input, SimpleItem };

use error::ComponentError::InvalidMessageId;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MessageID {
    message_id: SimpleItem
}


impl MessageID {

    //FIXME make into AsRef<str> for MessageID
    pub fn as_str( &self ) -> &str {
        self.message_id.as_str()
    }
}

impl FromInput for MessageID {
    fn from_input<I: Into<Input>>( input: I ) ->  Result<Self> {
        use self::parser_parts::parse_message_id;
        let input = input.into();

        match parse_message_id( &**input ) {
            IResult::Done( "", _msg_id ) => {},
            other => bail!(InvalidMessageId(input.as_str().to_owned(), other))
        }


        Ok( MessageID { message_id: input.into() } )
    }
}

impl EncodableInHeader for  MessageID {

    fn encode(&self, handle: &mut EncodeHandle) -> Result<()> {
        handle.mark_fws_pos();
        handle.write_char( SoftAsciiChar::from_char_unchecked('<') )?;
        match self.message_id {
            SimpleItem::Ascii( ref ascii ) => handle.write_str( ascii )?,
            SimpleItem::Utf8( ref utf8 ) => handle.write_utf8( utf8 )?
        }
        handle.write_char( SoftAsciiChar::from_char_unchecked('>') )?;
        handle.mark_fws_pos();
        Ok( () )
    }
}

#[derive(Debug)]
pub struct MessageIDList( pub Vec1<MessageID> );

deref0!{ +mut MessageIDList => Vec1<MessageID> }

impl EncodableInHeader for  MessageIDList {

    fn encode(&self, handle: &mut EncodeHandle) -> Result<()> {
        for msg_id in self.iter() {
            msg_id.encode( handle )?;
        }
        Ok( () )
    }
}


//NOTE for parsing mails we have to make sure to _require_ '<>' around the email

#[cfg(test)]
mod test {
    use core::grammar::MailType;
    use core::codec::{ Encoder, VecBodyBuf };
    use super::*;

    ec_test!{ simple, {
        MessageID::from_input( "affen@haus" )?
    } => ascii => [
        MarkFWS,
        // there are two "context" one which allows FWS inside (defined = email)
        // and one which doesn't for simplicity we use the later every where
        // especially because message ids without a `@domain.part` are quite
        // common
        Text "<affen@haus>",
        MarkFWS
    ]}

    ec_test!{ utf8, {
        MessageID::from_input( "↓@↑.utf8")?
    } => utf8 => [
        MarkFWS,
        Text "<↓@↑.utf8>",
        MarkFWS
    ]}

    #[test]
    fn utf8_fails() {
        let mut encoder = Encoder::<VecBodyBuf>::new(MailType::Ascii);
        let mut handle = encoder.encode_handle();
        let mid = MessageID::from_input( "abc@øpunny.code" ).unwrap();
        assert_err!(mid.encode( &mut handle ));
        handle.undo_header();
    }

    ec_test!{ multipls, {
        let fst = MessageID::from_input( "affen@haus" )?;
        let snd = MessageID::from_input( "obst@salat" )?;
        MessageIDList( vec1! [
            fst,
            snd
        ])
    } => ascii => [
        MarkFWS,
        Text "<affen@haus>",
        MarkFWS,
        MarkFWS,
        Text "<obst@salat>",
        MarkFWS,
    ]}
}

mod parser_parts {
    use nom::IResult;
    use core::grammar::{is_atext, is_dtext, MailType };

    pub fn parse_message_id( input: &str) -> IResult<&str, (&str, &str)> {
        do_parse!( input,
            l: id_left >>
            char!( '@' ) >>
            r: id_right >>
            (l, r)
        )
    }

    #[inline(always)]
    pub fn id_left( input: &str ) -> IResult<&str, &str> {
        dot_atom_text( input )
    }

    pub fn id_right( input: &str ) -> IResult<&str, &str> {
        alt!(
            input,
            no_fold_literal |
            dot_atom_text
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
        recognize!( input, tuple!(
            take_while1!( call!( is_atext, MailType::Internationalized ) ),
            many0!(tuple!(
                char!( '.' ),
                take_while1!( call!( is_atext, MailType::Internationalized ) )
            ))
        ) )
    }

    #[cfg(test)]
    mod test {
        use nom;
        use super::*;

        #[test]
        fn rec_dot_atom_text_no_dot() {
            match dot_atom_text( "abc" ) {
                IResult::Done( "", "abc" ) => {},
                other  => panic!("excepted Done(\"\",\"abc\") got {:?}", other )
            }
        }

        #[test]
        fn rec_dot_atom_text_dots() {
            match dot_atom_text( "abc.def.ghi" ) {
                IResult::Done( "", "abc.def.ghi" ) => {},
                other  => panic!("excepted Done(\"\",\"abc.def.ghi\") got {:?}", other )
            }
        }

        #[test]
        fn rec_dot_atom_text_no_end_dot() {
            let test_str = "abc.";
            let need_size = test_str.len() + 1;
            match dot_atom_text( test_str ) {
                IResult::Incomplete( nom::Needed::Size( ns ) ) if ns == need_size => {}
                other  => panic!("excepted Incomplete(Complete) got {:?}", other )
            }
        }

        #[test]
        fn rec_dot_atom_text_no_douple_dot() {
            match dot_atom_text( "abc..de" ) {
                IResult::Done( "..de", "abc" ) => {},
                other  => panic!( "excepted Done(\"..de\",\"abc\") got {:?}", other )
            }
        }

        #[test]
        fn rec_dot_atom_text_no_start_dot() {
            match dot_atom_text( ".abc" ) {
                IResult::Error( .. ) => {},
                other => panic!( "expected error got {:?}", other )
            }
        }



        #[test]
        fn no_empty() {
            match dot_atom_text( "" ) {
                IResult::Incomplete( nom::Needed::Size( 1 ) ) => {},
                other => panic!( "excepted Incomplete(Size(1)) got {:?}", other )
            }
        }
    }
}





use ascii::AsciiChar;

use nom::IResult;

use error::*;
use utils::Vec1;
use codec::{ MailEncoder, MailEncodable };

use data::{ FromInput, Input, SimpleItem };

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize)]
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
            other => bail!( "invalid message id \"{:?}\" (parse result: {:?})", input, other )
        }


        Ok( MessageID { message_id: input.into() } )
    }
}

impl<E> MailEncodable<E> for MessageID where E: MailEncoder {

    fn encode(&self, encoder: &mut E) -> Result<()> {
        encoder.note_optional_fws();
        encoder.write_char( AsciiChar::LessThan );
        match self.message_id {
            SimpleItem::Ascii( ref ascii ) => encoder.write_str( ascii ),
            SimpleItem::Utf8( ref utf8 ) => encoder.try_write_utf8( utf8 )?
        }
        encoder.write_char( AsciiChar::GreaterThan );
        encoder.note_optional_fws();
        Ok( () )
    }
}

pub struct MessageIDList( pub Vec1<MessageID> );

deref0!{ +mut MessageIDList => Vec1<MessageID> }

impl<E> MailEncodable<E> for MessageIDList where E: MailEncoder {

    fn encode(&self, encoder: &mut E) -> Result<()> {
        for msg_id in self.iter() {
            msg_id.encode( encoder )?;
        }
        Ok( () )
    }
}


//NOTE for parsing mails we have to make sure to _require_ '<>' around the email

#[cfg(test)]
mod test {
    use grammar::MailType;
    use codec::test_utils::*;
    use super::*;

    ec_test!{ simple, {
        MessageID::from_input( "affen@haus" )
    } => ascii => [
        OptFWS,
        LinePart( "<affen@haus>" ),
        OptFWS
    ]}

    ec_test!{ utf8, {
        MessageID::from_input( "↓@↑.utf8")
    } => utf8 => [
        OptFWS,
        LinePart( "<↓@↑.utf8>" ),
        OptFWS
    ]}

    #[test]
    fn utf8_fails() {
        let mut ec = TestMailEncoder::new( MailType::Ascii );
        let mid = MessageID::from_input( "abc@øpunny.code" ).unwrap();
        let res = mid.encode( &mut ec );
        assert_eq!( false, res.is_ok() );
    }

    ec_test!{ multipls, {
        let fst = MessageID::from_input( "affen@haus" ).unwrap();
        let snd = MessageID::from_input( "obst@salat" ).unwrap();
        Some( MessageIDList( vec1! [
            fst,
            snd
        ]))
    } => ascii => [
        OptFWS,
        LinePart( "<affen@haus>" ),
        OptFWS,
        OptFWS,
        LinePart( "<obst@salat>" ),
        OptFWS
    ]}
}

mod parser_parts {
    use nom::IResult;
    use grammar::{is_atext, is_dtext, MailType };

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




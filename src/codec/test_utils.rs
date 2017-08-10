#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use std::mem;

use ascii::{ AsciiChar,  AsciiStr };

use error::*;
use grammar::{is_atext, MailType };

use super::traits::MailEncoder;


#[macro_export]
macro_rules! ec_test {
    ( $name:ident, $inp:block => $mt:tt => [ $($state:expr),* ] ) => (
        #[test]
        fn $name() {
            use codec::MailEncodable;
            use codec::test_utils::TestMailEncoder;
            use codec::test_utils::State;

            let mt_str = stringify!($mt).to_uppercase();
            let mt = match mt_str.as_str() {
                "UTF8" => $crate::grammar::MailType::Internationalized,
                "ASCII" =>  $crate::grammar::MailType::Ascii,
                other => panic!( "invalide string for mail type: {}", other)
            };
            let mut ec = TestMailEncoder::new(mt);
            let to_encode = $inp.unwrap();
            to_encode.encode( &mut ec ).unwrap();
            let exp: Vec<State> =  vec![
                $($state),*
            ];
            assert_eq!( exp, ec.into_state_seq() );
        }
    );
}


#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub enum State {
    Line(String),
    LinePart(String),
    FWS,
    OptFWS,
    Body(String)
}

pub fn Line<S: Into<String>>(data: S) -> State {
    State::Line(data.into())
}
pub fn LinePart<S: Into<String>>(data: S) -> State {
    State::LinePart(data.into())
}
pub fn Body<S: Into<String>>(data: S) -> State {
    State::Body(data.into())
}


pub const FWS: State = State::FWS;
pub const OptFWS: State = State::OptFWS;



#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct TestMailEncoder {
    mail_type: MailType,
    current_line: String,
    state_seq: Vec<State>
}

impl TestMailEncoder {
    pub fn new( mail_type: MailType ) -> Self {
        TestMailEncoder {
            mail_type,
            current_line: String::new(),
            state_seq: Vec::new()
        }
    }

    fn push_line_part( &mut self )  {
        if self.current_line.len() > 0 {
            let line = mem::replace( &mut self.current_line, String::new() );
            self.state_seq.push( State::LinePart( line ) );
        }
    }

    pub fn into_state_seq( self ) -> Vec<State> {
        self.into()
    }
}

impl Into<Vec<State>> for TestMailEncoder {

    fn into(mut self) -> Vec<State> {
        self.push_line_part();
        self.state_seq
    }
}

impl MailEncoder for TestMailEncoder {
    fn mail_type( &self ) -> MailType {
        self.mail_type
    }

    fn write_new_line( &mut self ) {
        let line = mem::replace( &mut self.current_line, String::new() );
        self.state_seq.push( State::Line( line ) )
    }
    fn write_fws( &mut self ) {
        self.push_line_part();
        self.state_seq.push( State::FWS );
    }

    fn note_optional_fws(&mut self ) {
       self.push_line_part();
        self.state_seq.push( State::OptFWS );
    }

    fn write_char( &mut self, char: AsciiChar ) {
        self.current_line.push( char.as_char() );
    }

    fn write_str( &mut self, str: &AsciiStr ) {
        self.current_line += str.as_str()
    }

    fn try_write_utf8( &mut self, data: &str ) -> Result<()> {
        if self.mail_type.supports_utf8() {
            self.current_line += data;
            Ok( () )
        } else {
            bail!( "[test] trying to write utf8 on ascii encoder: {:?}", data )
        }
    }

    fn try_write_atext( &mut self, word: &str ) -> Result<()> {
        if word.chars().all( |ch| is_atext( ch, self.mail_type() ) ) {
            self.current_line += word;
            Ok( () )
        } else {
            bail!( "can not write atext, input is not valid atext" );
        }
    }


    /// writes a string to the encoder without checking if it is compatible
    /// with the mail type, if not used correctly this can write Utf8 to
    /// an Ascii Mail, which is incorrect but has to be safe wrt. rust's safety.
    fn write_str_unchecked( &mut self, data: &str) {
        self.current_line += data;
    }



    fn current_line_byte_length(&self ) -> usize {
        self.current_line.len()
    }

    //could also be called write_data_unchecked
    fn write_body( &mut self, body: &[u8]) {
        use std::str;
        self.push_line_part();
        let body: String = if self.mail_type.supports_utf8() {
            str::from_utf8( body )
                .expect( "bodies to only contain utf8 for now" )
                .into()
        } else {
            AsciiStr::from_ascii( body )
                .expect( "bodies in non internation mail can only contain 7bit ascii" )
                .as_str()
                .into()
        };
        self.state_seq.push( State::Body( body ) );
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn empty() -> Vec<State> {
        Vec::new()
    }

    #[test]
    fn push_line_part__empty() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.push_line_part();
        assert_eq!( empty(), ec.into_state_seq() )
    }

    #[test]
    fn push_line_part__nonempty() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.write_str( AsciiStr::from_ascii( "hy there" ).unwrap() );
        ec.push_line_part();
        assert_eq!( vec![
            LinePart( "hy there" )
        ], ec.into_state_seq() )
    }

    #[test]
    fn push_line_part__again_empty() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.write_str( AsciiStr::from_ascii( "hy there" ).unwrap() );
        ec.write_new_line();
        ec.push_line_part();
        assert_eq!( vec![
            Line( "hy there" )
        ], ec.into_state_seq() )
    }

    #[test]
    fn into_state_seq() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.write_str( AsciiStr::from_ascii( "hy there" ).unwrap() );
        ec.write_new_line();
        ec.write_str( AsciiStr::from_ascii( "abc" ).unwrap() );
        assert_eq!( vec![
            Line( "hy there" ),
            LinePart( "abc" )
        ], ec.into_state_seq() )
    }


    #[test]
    fn write_new_line() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.write_new_line();
        assert_eq!( vec![
            Line( "" )
        ], ec.into_state_seq() )
    }

    #[test]
    fn write_fws() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.write_str( AsciiStr::from_ascii( "part" ).unwrap() );
        ec.write_fws();
        ec.write_str( AsciiStr::from_ascii( "part2" ).unwrap() );
        assert_eq!( vec![
            LinePart( "part" ),
            FWS,
            LinePart( "part2" ),
        ], ec.into_state_seq() )
    }

    #[test]
    fn note_fws() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.write_str( AsciiStr::from_ascii( "part" ).unwrap() );
        ec.note_optional_fws();
        ec.write_str( AsciiStr::from_ascii( "part2" ).unwrap() );
        assert_eq!( vec![
            LinePart( "part" ),
            OptFWS,
            LinePart( "part2" ),
        ], ec.into_state_seq() )
    }

    #[test]
    fn write_str() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.write_str( AsciiStr::from_ascii( "part" ).unwrap() );
        assert_eq!( vec![
            LinePart( "part" ),
        ], ec.into_state_seq() )
    }

    #[test]
    fn write_char() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.write_char( AsciiChar::At );
        assert_eq!( vec![
            LinePart( "@" ),
        ], ec.into_state_seq() )
    }

    #[test]
    fn write_mixed() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.write_str( ascii_str!{ a b c } );
        ec.write_char( AsciiChar::At );
        ec.write_str( ascii_str!{ d Dot e } );
        assert_eq!( vec![
            LinePart( "abc@d.e" ),
        ], ec.into_state_seq() )
    }

    #[test]
    fn try_write_utf8__nok() {
        let mut ec = TestMailEncoder::new( MailType::Ascii );
        assert_eq!( false, ec.try_write_utf8("").is_ok() )
    }

    #[test]
    fn try_write_utf8__ok() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.try_write_utf8( "↑↓↑↓" ).unwrap();
        assert_eq!( vec![
            LinePart( "↑↓↑↓" ),
        ], ec.into_state_seq() )
    }

    #[test]
    fn try_write_atext__nok() {
        let mut ec = TestMailEncoder::new( MailType::Ascii );
        assert_eq!( false, ec.try_write_atext("↑↓↑↓").is_ok() );
    }

    #[test]
    fn try_write_atext__ok() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.try_write_atext( "↑↓↑↓" ).unwrap();
        assert_eq!( vec![
            LinePart( "↑↓↑↓" ),
        ], ec.into_state_seq() )
    }

    #[test]
    fn try_write_atext__nok2() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        assert_eq!( false, ec.try_write_atext(" ").is_ok() );
    }

    #[test]
    fn write_body__utf8() {
        let mut ec = TestMailEncoder::new( MailType::Internationalized );
        ec.write_body( "→ →".as_bytes() );
        assert_eq!( vec![
            Body( "→ →" )
        ], ec.into_state_seq() )
    }

    #[should_panic]
    #[test]
    fn write_body__ascii_nok() {
        let mut ec = TestMailEncoder::new( MailType::Ascii );
        ec.write_body( "→ →".as_bytes() );
    }

    #[test]
    fn write_body__ascii_ok() {
        let mut ec = TestMailEncoder::new( MailType::Ascii );
        ec.write_body( "abc".as_bytes() );
        assert_eq!( vec![
            Body( "abc" )
        ], ec.into_state_seq() )
    }
}
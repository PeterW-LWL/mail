use error::*;
use grammar::MailType;
use ascii::{  AsciiStr, AsciiChar };

pub trait EncodedWordWriter {
    fn write_char( &mut self, ch: AsciiChar );

    fn write_ecw_start( &mut self );
    fn write_ecw_end( &mut self );
    fn write_ecw_seperator( &mut self );
    fn max_payload_len( &self ) -> usize;

    fn start_new_encoded_word( &mut self ) -> usize {
        self.write_ecw_end();
        self.write_ecw_seperator();
        self.write_ecw_start();
        self.max_payload_len()
    }
}

pub trait MailEncoder {
    fn mail_type( &self ) -> MailType;

    fn write_new_line( &mut self );
    fn write_fws( &mut self );
    fn note_optional_fws(&mut self );

    fn write_char( &mut self, char: AsciiChar );
    fn write_str( &mut self, str: &AsciiStr );

    fn try_write_utf8( &mut self, str: &str ) -> Result<()>;
    fn try_write_atext( &mut self, str: &str ) -> Result<()>;
    //fn write_encoded_word( &mut self, data: &str, ctx: EncodedWordContext );

    /// writes a string to the encoder without checking if it is compatible
    /// with the mail type, if not used correctly this can write Utf8 to
    /// an Ascii Mail, which is incorrect but has to be safe wrt. rust's safety.
    fn write_str_unchecked( &mut self, str: &str);


    fn current_line_byte_length(&self ) -> usize;

    //could also be called write_data_unchecked
    fn write_body( &mut self, body: &[u8]);
}


pub trait MailEncodable {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()>
        where E: MailEncoder;
}

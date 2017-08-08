use ascii::{ AsciiString, AsciiChar, AsciiStr };

use types::Vec1;
use grammar::encoded_word::{ MAX_ECW_LEN, ECW_SEP_OVERHEAD };
use data::encoded_word::Encoding;
use super::traits::MailEncoder;
use super::traits::EncodedWordWriter;

pub struct VecWriter<'a> {
    data: Vec1<AsciiString >,
    charset: &'a AsciiStr,
    encoding: Encoding
}

impl<'a> VecWriter<'a> {
    pub fn new(charset: &'a AsciiStr, encoding: Encoding) -> Self {
        let data = Vec1::new( AsciiString::new() );
        VecWriter { data, charset, encoding }
    }

    pub fn data( &self ) -> &[AsciiString] {
        &*self.data
    }
}

impl<'a> Into<Vec1<AsciiString>> for VecWriter<'a> {
    fn into(self) -> Vec1<AsciiString> {
        self.data
    }
}

impl<'a> EncodedWordWriter for VecWriter<'a> {

    fn encoding( &self ) -> Encoding {
        self.encoding
    }

    fn write_char( &mut self, ch: AsciiChar ) {
        self.data.last_mut().push( ch );
    }

    fn write_charset( &mut self ) {
        self.data.last_mut().extend( self.charset.chars() )
    }

    fn write_ecw_seperator( &mut self ) {
        self.data.push( AsciiString::new() )
    }

    fn max_payload_len( &self ) -> usize {
        MAX_ECW_LEN - ECW_SEP_OVERHEAD - self.charset.len() - 1
    }
}

pub struct WriterWrapper<'a, E:'a>{
    charset: &'a AsciiStr,
    encoding: Encoding,
    encoder: &'a mut E
}

impl<'a, E> WriterWrapper<'a, E> where E: MailEncoder + 'a {
    pub fn new( charset: &'a AsciiStr, encoding: Encoding, encoder: &'a mut E ) -> Self {
        WriterWrapper { charset, encoding, encoder }
    }
}

impl<'a, E> EncodedWordWriter for WriterWrapper<'a, E> where E: MailEncoder + 'a {

    fn encoding( &self ) -> Encoding {
        self.encoding
    }

    fn write_charset( &mut self ) {
        self.encoder.write_str( self.charset )
    }

    fn write_ecw_seperator( &mut self ) {
        self.encoder.write_fws();
    }

    fn write_char( &mut self, ch: AsciiChar ) {
        self.encoder.write_char( ch )
    }

    fn max_payload_len( &self ) -> usize {
        MAX_ECW_LEN - ECW_SEP_OVERHEAD - self.charset.len() - 1
    }
}

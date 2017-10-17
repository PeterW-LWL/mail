use soft_ascii_string::{ SoftAsciiString, SoftAsciiChar, SoftAsciiStr };

use external::vec1::Vec1;
use grammar::encoded_word::{ MAX_ECW_LEN, ECW_SEP_OVERHEAD };
use super::{ EncodedWordEncoding as Encoding };
use super::encoder::EncodeHandle;
use super::traits::EncodedWordWriter;

pub struct VecWriter<'a> {
    data: Vec1<SoftAsciiString >,
    charset: &'a SoftAsciiStr,
    encoding: Encoding
}

impl<'a> VecWriter<'a> {
    pub fn new(charset: &'a SoftAsciiStr, encoding: Encoding) -> Self {
        let data = Vec1::new( SoftAsciiString::new() );
        VecWriter { data, charset, encoding }
    }

    pub fn data( &self ) -> &[SoftAsciiString] {
        &*self.data
    }
}

impl<'a> Into<Vec1<SoftAsciiString>> for VecWriter<'a> {
    fn into(self) -> Vec1<SoftAsciiString> {
        self.data
    }
}

impl<'a> EncodedWordWriter for VecWriter<'a> {

    fn encoding( &self ) -> Encoding {
        self.encoding
    }

    fn write_char( &mut self, ch: SoftAsciiChar ) {
        self.data.last_mut().push( ch );
    }

    fn write_charset( &mut self ) {
        self.data.last_mut().extend( self.charset.chars() )
    }

    fn write_ecw_seperator( &mut self ) {
        self.data.push( SoftAsciiString::new() )
    }

    fn max_payload_len( &self ) -> usize {
        MAX_ECW_LEN - ECW_SEP_OVERHEAD - self.charset.len() - 1
    }
}

pub struct WriterWrapper<'a, 'b: 'a>{
    charset: &'a SoftAsciiStr,
    encoding: Encoding,
    encoder_handle: &'a mut EncodeHandle<'b>
}

impl<'a, 'b: 'a> WriterWrapper<'a, 'b> {
    pub fn new(charset: &'a SoftAsciiStr,
               encoding: Encoding,
               encoder: &'a mut EncodeHandle<'b> ) -> Self
    {
        WriterWrapper { charset, encoding, encoder_handle: encoder }
    }
}

impl<'a, 'b: 'a> EncodedWordWriter for WriterWrapper<'a, 'b> {

    fn encoding( &self ) -> Encoding {
        self.encoding
    }

    fn write_charset( &mut self ) {
        //TODO fix
        let _ = self.encoder_handle.write_str( self.charset );
    }

    fn write_ecw_seperator( &mut self ) {
        self.encoder_handle.write_fws();
    }

    fn write_char( &mut self, ch: SoftAsciiChar ) {
        //TODO fix
        let _ = self.encoder_handle.write_char( ch );
    }

    fn max_payload_len( &self ) -> usize {
        MAX_ECW_LEN - ECW_SEP_OVERHEAD - self.charset.len() - 1
    }
}

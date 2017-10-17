use soft_ascii_string::{ SoftAsciiStr, SoftAsciiChar };

use error::*;

use super::EncodedWordEncoding;

pub trait EncodedWordWriter {
    fn write_char( &mut self, ch: SoftAsciiChar );
    fn write_charset( &mut self );
    fn encoding( &self ) -> EncodedWordEncoding;
    fn write_ecw_seperator( &mut self );

    /// Returns the maximal length of the paylod/encoded data
    ///
    /// Any number of calls to methods on in trait in any way
    /// should never be able to change the returned value.
    /// Only changing e.g. the charset or encoding should be
    /// able to change what `max_paylod_len` returns.
    fn max_payload_len( &self ) -> usize;

    fn write_ecw_start( &mut self ) {
        let qm = SoftAsciiChar::from_char_unchecked('?');
        self.write_char(SoftAsciiChar::from_char_unchecked('='));
        self.write_char(qm);
        self.write_charset();
        self.write_char(qm);
        let acronym = self.encoding().acronym();
        self.write_str( acronym );
        self.write_char(qm);
    }

    fn write_ecw_end( &mut self ) {
        self.write_char( SoftAsciiChar::from_char_unchecked('?') );
        self.write_char( SoftAsciiChar::from_char_unchecked('=') );
    }


    fn start_next_encoded_word( &mut self )  {
        self.write_ecw_end();
        self.write_ecw_seperator();
        self.write_ecw_start();
    }

    fn write_str( &mut self, s: &SoftAsciiStr ) {
        for ch in s.chars() {
            self.write_char(ch)
        }
    }
}


/// Trait Repesenting the buffer of a mime body payload
///
/// (e.g. a transfer encoded image or text)
///
/// Note that the `BodyBuffer` trait is mainly used to break a
/// cyclic dependency between `codec` and `mail::resource`.
/// So while all code in lower layers is generic over _one_
/// kind of BodyBuffer for all Buffers the higher layers
/// in `mail` and `mail_composition` are fixed on `Resource`.
///
pub trait BodyBuffer {

    /// Called to access the bytes in the buffer.
    ///
    /// By limiting the access to a closure passed in
    /// it enables a number of properties for implementators:
    /// - the byte slice has only to be valid for the duration of the closure,
    ///   allowing implementations for data behind a Lock which has to keep
    ///   a Guard alive during the access of the data
    /// - the implementor can directly return a error if for some
    ///   reason no data is available or the data was "somehow" corrupted
    fn with_slice<FN, R>(&self, func: FN) -> Result<R>
        where FN: FnOnce(&[u8]) -> Result<R>;
}
use std::ops::Deref;

use codec::{base64, quoted_printable};
use soft_ascii_string::SoftAsciiStr;
use error::*;
use utils::FileBuffer;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum TransferEncoding {
    _7Bit,
    _8Bit,
    Binary,
    QuotedPrintable,
    Base64,
    Other( Token ),
}

impl TransferEncoding {
    pub fn repr(&self ) -> &SoftAsciiStr {
        use self::TransferEncoding::*;
        match *self {
            _7Bit => SoftAsciiStr::from_str_unchecked("7bit"),
            _8Bit => SoftAsciiStr::from_str_unchecked("8bit"),
            Binary =>  SoftAsciiStr::from_str_unchecked("binary"),
            QuotedPrintable => SoftAsciiStr::from_str_unchecked("quoted-printable"),
            Base64 =>  SoftAsciiStr::from_str_unchecked("base64"),
            Other( ref _token ) =>
                unreachable!("token is not implemented and not constructable from outside")
        }
    }
}

///Token is either a ietf-token or an x-token, but it's usage is not yet supported
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Token { _nop: () }



pub fn find_encoding( buffer: &FileBuffer ) -> TransferEncoding {
    if buffer.has_ascii_charset() {
        //TODO support lossy 7Bit encoding dropping '\0' and orphan '\n', '\r'
        TransferEncoding::_7Bit
    } else if buffer.contains_text() {
        TransferEncoding::QuotedPrintable
    } else {
        TransferEncoding::Base64
    }
}



#[derive(Debug, Clone)]
pub struct TransferEncodedFileBuffer {
    inner: FileBuffer,
    encoding: TransferEncoding
}

impl TransferEncodedFileBuffer {
    pub fn buffer_is_encoded( buf: FileBuffer, with_encoding: TransferEncoding ) -> Self {
        TransferEncodedFileBuffer {
            inner: buf,
            encoding: with_encoding
        }
    }

    pub fn transfer_encoding( &self ) -> &TransferEncoding {
        &self.encoding
    }

    /// transforms a unencoded FileBuffer into a TransferEncodedFileBuffer
///
/// if a preferred_encoding is given it is used,
/// else if the buffer has a ascii charset 7Bit encoding is used
/// else if the buffer contains text quoted-printable is used
/// else base64 encoding is used
    pub fn encode_buffer(
        buffer: FileBuffer,
        preferred_encoding: Option<&TransferEncoding>
    ) -> Result<TransferEncodedFileBuffer>
    {
        use self::TransferEncoding::*;
        let encoding;
        let encoding_ref;

        if let Some( enc_ref ) = preferred_encoding {
            encoding_ref = enc_ref;
        } else {
            encoding = find_encoding( &buffer );
            encoding_ref = &encoding;
        }

        match *encoding_ref {
            _7Bit => encode_7bit( buffer ),
            _8Bit => encode_8bit( buffer ),
            Binary => encode_binary( buffer ),
            QuotedPrintable => encode_quoted_printable( buffer ),
            Base64 => encode_base64( buffer ),
            //FEATURE_TODO(non_standart_encoding): check if the encoding is in a ~singelton registry
            Other( ref token ) => bail!( "only standart encodings are supported, got: {:?}", token )
        }
    }

}


impl Deref for TransferEncodedFileBuffer {
    type Target = FileBuffer;
    fn deref( &self ) -> &FileBuffer {
        &self.inner
    }
}



fn encode_7bit( buffer: FileBuffer ) -> Result<TransferEncodedFileBuffer> {
    {
        let data: &[u8] = &*buffer;

        let mut last = b'\0';
        for byte in data.iter().cloned() {
            if byte >= 128 || byte == 0 {
                return Err( ErrorKind::Invalide7BitValue( byte ).into() )
            }
            if ( last==b'\r' ) != (byte == b'\n') {
                return Err( ErrorKind::Invalide7BitSeq( byte ).into() )
            }
            last = byte;
        }
    }
    Ok( TransferEncodedFileBuffer::buffer_is_encoded( buffer, TransferEncoding::_7Bit ) )
}

fn encode_8bit( buffer: FileBuffer ) -> Result<TransferEncodedFileBuffer> {
    {
        let data: &[u8] = &*buffer;

        let mut last = b'\0';
        for byte in data.iter().cloned() {
            if  byte == 0 {
                bail!( ErrorKind::Invalide8BitValue( byte ) );
            }
            if ( last==b'\r' ) != (byte == b'\n') {
                bail!( ErrorKind::Invalide8BitSeq( byte ) );
            }
            last = byte;
        }
    }
    Ok( TransferEncodedFileBuffer::buffer_is_encoded( buffer, TransferEncoding::_8Bit ) )
}

/// to quote RFC 2045:
/// """[at time of writing] there are no standardized Internet mail transports
///    for which it is legitimate to include
///    unencoded binary data in mail bodies. [...]"""
///
/// nevertheless there is at last one SMTP extension which allows this
/// (chunked),but this library does not support it for now
fn encode_binary( buffer: FileBuffer ) -> Result<TransferEncodedFileBuffer> {
    Ok( TransferEncodedFileBuffer::buffer_is_encoded( buffer, TransferEncoding::Binary ) )
}

fn encode_quoted_printable( buffer: FileBuffer ) -> Result<TransferEncodedFileBuffer> {
    Ok( TransferEncodedFileBuffer::buffer_is_encoded(
        buffer.with_data( |data| quoted_printable::normal_encode( data ).into() ),
        TransferEncoding::QuotedPrintable
    ) )
}

fn encode_base64( buffer: FileBuffer ) -> Result<TransferEncodedFileBuffer> {
    Ok( TransferEncodedFileBuffer::buffer_is_encoded(
        buffer.with_data( |data| base64::normal_encode(data).into_bytes() ),
        TransferEncoding::Base64
    ) )
}


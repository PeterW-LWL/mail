use ascii::AsciiStr;

use error::*;
use codec::{ MailEncoder, MailEncodable };

pub use types::DateTime;

impl MailEncodable for DateTime {
    fn encode<E>( &self, encoder:  &mut E ) -> Result<()> where E: MailEncoder {
        let as_str = self.to_rfc2822();
        let ascii = unsafe { AsciiStr::from_ascii_unchecked( &*as_str ) };
        encoder.write_str( ascii );
        Ok( () )
    }
}
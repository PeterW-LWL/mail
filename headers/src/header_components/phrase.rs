

use internals::{
    error::EncodingError,
    encoder::{EncodingWriter, EncodableInHeader},
    bind::encoded_word::{EncodedWordEncoding, WriterWrapper}
};

use crate::{
    error::ComponentCreationError,
    HeaderTryFrom
};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Phrase {
    input: String,
}

impl Phrase {

    pub fn new<I>(input: I) -> Self
        where I: Into<String>
    {
        Phrase { input: input.into() }
    }
}

impl From<String> for Phrase {
    fn from(input: String) -> Self {
        Phrase::new(input)
    }
}

impl<'a> From<&'a str> for Phrase {
    fn from(input: &'a str) -> Self {
        Phrase::new(input)
    }
}

impl HeaderTryFrom<String> for Phrase {
    fn try_from(input: String) -> Result<Self, ComponentCreationError> {
        Ok(Phrase::new(input))
    }
}

impl<'a> HeaderTryFrom<&'a str> for Phrase {
    fn try_from(input: &'a str) -> Result<Self, ComponentCreationError> {
        Ok(Phrase::new(input))
    }
}

impl EncodableInHeader for  Phrase {

    /* (This is intentionally not rust-doc, it's notes for devs working on/changing the
        encoding algorithm, i.e. notes about internal implementation details, and potentially
        outdated, too.)

        grammar:
            word     = atom    / quoted-string
            phrase   = 1*word  / obs-phrase
            atom     = [CFWS] 1*atext [CFWS]

        context:
            if possible display all without any quoting/encoding (optional)
            if not try to place it into one quote string
            preferable make it still as much "human-readable" as possible
            do not have `<enc-word><FWS><enc-word>` (expect if you break the line)
            do not have a line length > 76 (RFC2047 bs.)

        algorithm:
            1. try encode it as one quoted string
            2. if not possible encoded it as a sequence of quoted printable encoded words
                - this can be improved on, as we might only want to encode what really is
                  necessary, but we also want to encode whole words, but for some inputs
                  word splitting is non-trivial and we don't want to have some "random letter"
                  in quoted string between encoded words, and we then also have to care about
                  non-semantic LWS between two encoded words, so we would need a algorithm
                  which tries to split words or falls back to just using encoded words etc.
    */
    fn encode(&self, handle: &mut EncodingWriter) -> Result<(), EncodingError> {
        handle
            .try_write_quoted_string(&self.input)
            .handle_condition_failure(|handle| {
                let encoding = EncodedWordEncoding::QuotedPrintable;
                let mut writer = WriterWrapper::new(
                    EncodedWordEncoding::QuotedPrintable,
                    handle
                );
                encoding.encode(&self.input, &mut writer);
                Ok(())
            })
    }

    fn boxed_clone(&self) -> Box<EncodableInHeader> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod test {
    use ::HeaderTryFrom;
    use super::Phrase;

    ec_test!{ simple, {
        Phrase::try_from("simple think")?
    } => ascii => [
        Text "\"simple",
        MarkFWS,
        Text " think\""
    ]}

    ec_test!{ with_encoding, {
        Phrase::try_from(" hm nääds encoding things is fun")?
    } => ascii => [
        Text "=?utf8?Q?=20hm=20n=C3=A4=C3=A4ds=20encoding=20things=20is=20fun?="
    ]}
}




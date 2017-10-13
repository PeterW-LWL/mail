use std::any::{Any, TypeId};
use std::fmt::{self, Debug};
use std::result::{ Result as StdResult };
use std::borrow::Cow;

use ascii::{AsciiStr, AsciiChar};

use error::Result;
use grammar::{is_atext, MailType};

use super::traits::BodyBuffer;

/// as specified in RFC 5322 not including CRLF
const LINE_LEN_SOFT_LIMIT: usize = 78;
/// as specified in RFC 5322 (mail) + RFC 5321 (smtp) not including CRLF
const LINE_LEN_HARD_LIMIT: usize = 998;

// can not be moved to `super::traits` as it depends on the
// EncodeHeaderHandle defined here
/// Trait Implemented by "components" used in header field bodies
///
/// This trait can be turned into a trait object allowing runtime
/// genericallity over the "components" if needed.
pub trait EncodableInHeader: Any+Debug {
    fn encode(&self, encoder:  &mut EncodeHeaderHandle) -> Result<()>;

    #[doc(hidden)]
    fn type_id( &self ) -> TypeId {
        TypeId::of::<Self>()
    }
}

//TODO we now could use MOPA or similar crates
impl EncodableInHeader {

    #[inline(always)]
    pub fn is<T: EncodableInHeader>(&self) -> bool {
        self.type_id() == TypeId::of::<T>()
    }


    #[inline]
    pub fn downcast_ref<T: EncodableInHeader>(&self) -> Option<&T> {
        if self.is::<T>() {
            Some( unsafe { &*( self as *const EncodableInHeader as *const T) } )
        } else {
            None
        }
    }

    #[inline]
    pub fn downcast_mut<T: EncodableInHeader>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            Some( unsafe { &mut *( self as *mut EncodableInHeader as *mut T) } )
        } else {
            None
        }
    }
}


pub trait EncodableInHeaderBoxExt: Sized {
    fn downcast<T: EncodableInHeader>(self) -> StdResult<Box<T>, Self>;
}

impl EncodableInHeaderBoxExt for Box<EncodableInHeader> {

    fn downcast<T: EncodableInHeader>(self) -> StdResult<Box<T>, Self> {
        if EncodableInHeader::is::<T>(&*self) {
            let ptr: *mut EncodableInHeader = Box::into_raw(self);
            Ok( unsafe { Box::from_raw(ptr as *mut T) } )
        } else {
            Err( self )
        }
    }
}

impl EncodableInHeaderBoxExt for Box<EncodableInHeader+Send> {

    fn downcast<T: EncodableInHeader>(self) -> StdResult<Box<T>, Self> {
        if EncodableInHeader::is::<T>(&*self) {
            let ptr: *mut EncodableInHeader = Box::into_raw(self);
            Ok( unsafe { Box::from_raw(ptr as *mut T) } )
        } else {
            Err( self )
        }
    }
}

/// Wrapper Struct to be able to use EncodableInHeader with Closures
///
/// Basically a workaround for Closures not implementing debug in all
/// circumstances and therefore ok to be placed in the `traits` module
/// as it is a replacement for a not so well working
/// `impl<FN> EncodableInHeader for FN where FN: ...`
pub struct EncodableClosure<F>(pub F);
impl<FN: 'static> EncodableInHeader for EncodableClosure<FN>
    where FN: for<'a,'b: 'a> Fn(&'a mut EncodeHeaderHandle<'b>) -> Result<()>
{
    fn encode(&self, encoder:  &mut EncodeHeaderHandle) -> Result<()> {
        (self.0)(encoder)
    }
}

impl<FN> Debug for EncodableClosure<FN> {

    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "EncodableClosure(..)")
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Section<R: BodyBuffer> {
    Header(String),
    MIMEBody(R)
}

impl<R> Section<R>
    where R: BodyBuffer
{
    pub fn unwrap_header(self) -> String {
        if let Section::Header(res) = self {
            res
        } else {
            panic!("expected `Section::Header` got `Section::Body`")
        }
    }
    pub fn unwrap_body(self) -> R {
        if let Section::MIMEBody(res) = self {
            res
        } else {
            panic!("expected `Section::MIMEBody` got `Section::Header`")
        }
    }
}


/// Encoder for a Mail providing a buffer for encodable traits
///
/// The buffer is a vector of section which either are string
/// buffers used to mainly encode headers or buffers of type R:BodyBuffer
/// which represent a valid body payload.
pub struct Encoder<R: BodyBuffer> {
    mail_type: MailType,
    sections: Vec<Section<R>>,
    #[cfg(test)]
    pub trace: Vec<Token>
}


/// If it is a test build the Encoder will
/// have an additional `pub trace` field,
/// which will contain a Vector of `Token`s
/// generated when writing to the string buffer.
///
/// For example when calling `.write_utf8("hy")`
/// following tokens will be added:
/// `[NowUtf8, Text("hy")]`
#[cfg(test)]
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Token {
    MarkFWS,
    CRLF,
    TruncateToCRLF,
    Text(String),
    NowChar,
    NowStr,
    NowAText,
    NowUtf8,
    NowUnchecked,
    NewSection,
    End,
    /// used to seperate a header from a body
    ///
    /// be aware that `Section::BodyPaylod` just
    /// contains the Payload, so e.g. headers from
    /// mime bodies or mime multipart body boundaries
    /// still get written into the string buffer
    BlankLine
}


impl<B: BodyBuffer> Encoder<B> {

    pub fn new(mail_type: MailType) -> Self {
        Encoder {
            mail_type,
            sections: Default::default(),
            #[cfg(test)]
            trace: Vec::new()
        }
    }

    pub fn mail_type( &self ) -> MailType {
        self.mail_type
    }

    /// returns a new EncodeHeaderHandle which contains
    /// a mutable reference to the current string buffer
    ///
    /// # Trace (test build only)
    /// pushes a `NewSection` Token if the the returned
    /// `EncodeHeaderHandle` refers to a new empty buffer
    pub fn encode_header_handle( &mut self ) -> EncodeHeaderHandle {
        if let Some(&Section::Header(..)) = self.sections.last() {}
        else {
            self.sections.push(Section::Header(String::new()));
            #[cfg(test)]
            { self.trace.push(Token::NewSection) }
        }

        if let Some(&mut Section::Header(ref mut string)) = self.sections.last_mut() {
            #[cfg(not(test))]
            { EncodeHeaderHandle::new(self.mail_type,  string) }
            #[cfg(test)]
            { EncodeHeaderHandle::new(self.mail_type,  string, &mut self.trace) }
        } else {
            //REFACTOR(NLL): with NLL we can combine both if-else blocks not needing unreachable! anymore
            unreachable!("we already made sure the last is Section::Header")
        }
    }

    pub fn add_blank_line(&mut self) {
        if let Some(&Section::Header(..)) = self.sections.last() {}
            else {
                self.sections.push(Section::Header(String::new()));
                #[cfg(test)]
                { self.trace.push(Token::NewSection); }
            }

        if let Some(&mut Section::Header(ref mut string)) = self.sections.last_mut() {
            string.push_str("\r\n");
            #[cfg(test)]
            { self.trace.push(Token::BlankLine); }
        } else {
            //REFACTOR(NLL): with NLL we can combine both if-else blocks not needing unreachable! anymore
            unreachable!("we already made sure the last is Section::Header")
        }
    }

    /// adds adds a body payload buffer to the encoder
    /// without validating it, the encoder mainly provides
    /// buffers it is not validating them.
    pub fn write_body( &mut self, body: B) {
        self.sections.push(Section::MIMEBody(body))
    }

    pub fn into_sections(self) -> Vec<Section<B>> {
        self.sections
    }

    //TODO other "write out" methods (iterator over &[u8]?)
    pub fn into_string_lossy(self) -> Result<String> {
        let mut out = String::new();
        for section in self.sections.into_iter() {
            match section {
                Section::Header(string) => {
                    out.push_str(&*string)
                },
                Section::MIMEBody(body) => {
                    body.with_slice(|slice| {
                        let mut string = String::from_utf8_lossy(slice);
                        out.push_str(&*string);
                        if !string.ends_with("\r\n") {
                            out.push_str("\r\n");
                        }
                        Ok(())
                    })?;
                }
            }
        }
        Ok(out)
    }
}


/// A handle providing method to write to the underlying buffer
/// keeping track of newlines the current line length and places
/// where the line can be broken so that the soft line length
/// limit (78) and the hard length limit (998) can be keept.
///
/// It's basically a string buffer which know how to brake
/// lines at the right place.
///
/// Note any act of writing a header through `EncodeHeaderHandle`
/// has to be concluded by either calling `finish` or `undo_header`.
/// If not this handle will panic when being dropped (and the thread
/// is not already panicing) as writes through the handle are directly
/// writes to the underlying buffer which now contains malformed/incomplete
/// data. (Note that this Handle does not own any Drop types so if realy
/// needed `forget`-ing it won't leak any memory)
///
pub struct EncodeHeaderHandle<'a> {
    buffer: &'a mut String,
    #[cfg(test)]
    trace: &'a mut Vec<Token>,
    mail_type: MailType,
    line_start_idx: usize,
    last_fws_idx: usize,
    skipped_cr: bool,
    /// if there had ben non WS chars since the last FWS
    /// or last line start, if there had been a line
    /// start since the last fws.
    content_since_fws: bool,
    /// represents if there had ben non WS chars before the last FWS
    /// on the current line (false if there was no FWS yet on the current
    /// line).
    content_before_fws: bool,
    header_start_idx: usize,
    #[cfg(test)]
    trace_start_idx: usize
}


impl<'a> Drop for EncodeHeaderHandle<'a> {

    fn drop(&mut self) {
        use std::thread;
        if !thread::panicking() &&  self.has_unfinished_parts() {
            // we really should panic as the back buffer i.e. the mail will contain
            // some partially written header which definitely is a bug
            panic!("dropped Handle which partially wrote header to back buffer (use `finish` or `discard`)")
        }
    }
}

impl<'a> EncodeHeaderHandle<'a> {

    #[cfg(not(test))]
    fn new(
        mail_type: MailType,
        buffer: &'a mut String,
    ) -> Self {
        let start_idx = buffer.len();
        EncodeHeaderHandle {
            buffer,
            mail_type,
            line_start_idx: start_idx,
            last_fws_idx: start_idx,
            skipped_cr: false,
            content_since_fws: false,
            content_before_fws: false,
            header_start_idx: start_idx
        }
    }

    #[cfg(test)]
    fn new(
        mail_type: MailType,
        buffer: &'a mut String,
        trace: &'a mut Vec<Token>
    ) -> Self {
        let start_idx = buffer.len();
        let trace_start_idx = trace.len();
        EncodeHeaderHandle {
            buffer,
            trace,
            mail_type,
            line_start_idx: start_idx,
            last_fws_idx: start_idx,
            skipped_cr: false,
            content_since_fws: false,
            content_before_fws: false,
            header_start_idx: start_idx,
            trace_start_idx
        }
    }

    fn reinit(&mut self) {
        let start_idx = self.buffer.len();
        self.line_start_idx = start_idx;
        self.last_fws_idx = start_idx;
        self.skipped_cr = false;
        self.content_since_fws = false;
        self.content_before_fws = false;
        self.header_start_idx = start_idx;
        #[cfg(test)]
        { self.trace_start_idx = self.trace.len(); }
    }

    #[inline]
    pub fn has_unfinished_parts(&self) -> bool {
        self.buffer.len() != self.header_start_idx
    }

    #[inline]
    pub fn mail_type(&self) -> MailType {
        self.mail_type
    }

    #[inline]
    pub fn line_has_content(&self) -> bool {
        self.content_before_fws | self.content_since_fws
    }

    #[inline]
    pub fn current_line_byte_length(&self) -> usize {
        self.buffer.len() - self.line_start_idx
    }

    /// marks the current position a a place where a soft
    /// line break (i.e. "\r\n ") can be inserted
    ///
    /// # Trace (test build only)
    /// does push a `MarkFWS` Token
    pub fn mark_fws_pos(&mut self) {
        #[cfg(test)]
        { self.trace.push(Token::MarkFWS) }
        self.content_before_fws |= self.content_since_fws;
        self.content_since_fws = false;
        self.last_fws_idx = self.buffer.len()
    }

    /// writes a ascii char to the underlying buffer
    ///
    /// # Error
    /// - fails if the hard line length limit is breached and the
    ///   line can not be broken with soft line breaks
    /// - buffer would contain a orphan '\r' or '\n' after the write
    ///
    /// # Trace (test build only)
    /// does push `NowChar` and then can push `Text`,`CRLF`
    pub fn write_char(&mut self, ch: AsciiChar) -> Result<()>  {
        #[cfg(test)]
        { self.trace.push(Token::NowChar) }
        self.internal_write_char(ch.as_char())
    }

    /// writes a ascii str to the underlying buffer
    ///
    /// # Error
    /// - fails if the hard line length limit is breached and the
    ///   line can not be broken with soft line breaks
    /// - buffer would contain a orphan '\r' or '\n' after the write
    ///
    /// Note that in case of an error part of the content might already
    /// have been written to the buffer, therefore it is recommended
    /// to call `undo_header` after an error (especially if the
    /// handle is doped after this!)
    ///
    /// # Trace (test build only)
    /// does push `NowStr` and then can push `Text`,`CRLF`
    ///
    pub fn write_str(&mut self, s: &AsciiStr)  -> Result<()>  {
        #[cfg(test)]
        { self.trace.push(Token::NowStr) }
        self.internal_write_str(s.as_str())
    }


    /// writes a utf8 str into a buffer for an internationalized mail
    ///
    /// # Error
    /// - fails if the underlying MailType is not Internationalized
    /// - fails if the hard line length limit is reached
    /// - buffer would contain a orphan '\r' or '\n' after the write
    ///
    /// Note that in case of an error part of the content might already
    /// have been written to the buffer, therefore it is recommended
    /// to call `undo_header` after an error (especially if the
    /// handle is doped after this!)
    ///
    /// # Trace (test build only)
    /// does push `NowUtf8` and then can push `Text`,`CRLF`
    pub fn write_utf8(&mut self, s: &str) -> Result<()> {
        if self.mail_type().is_internationalized() {
            #[cfg(test)]
            { self.trace.push(Token::NowUtf8) }
            self.internal_write_str(s)
        } else {
            bail!( "can not write utf8 into Ascii mail" )
        }
    }

    /// Writes a str assumed to be atext if it is atext given the mail type
    ///
    /// This method is mainly an optimazation as the "is atext" and is
    /// "is ascii if MailType is Ascii" aspects are checked at the same
    /// time resulting in a str which you know is ascii _if_ the mail
    /// type is Ascii and which might be non-us-ascii if the mail type
    /// is Inernationalized.
    ///
    /// # Error
    /// - if the text is not valid atext
    /// - if the MailType is not Inernationalized but it is only atext if it is
    ///   internationalized
    /// - if the hard line length limit is reached and the line can't be broken
    ///   with soft line breaks
    /// - buffer would contain a orphan '\r' or '\n' after the write
    ///
    /// Note that in case of an error part of the content might already
    /// have been written to the buffer, therefore it is recommended
    /// to call `undo_header` after an error (especially if the
    /// handle is doped after this!)
    ///
    /// # Trace (test build only)
    /// does push `NowAText` and then can push `Text`
    ///
    pub fn try_write_atext(&mut self, s: &str) -> Result<()> {
        if s.chars().all( |ch| is_atext( ch, self.mail_type() ) ) {
            #[cfg(test)]
            { self.trace.push(Token::NowAText) }
            // the ascii or not aspect is already coverted by `is_atext`
            self.internal_write_str(s)
        } else {
            bail!( "can not write atext, input is not valid atext" );
        }

    }

    //TODO remove once SoftAsciiString lands
    /// writes a string to the encoder without checking if it is compatible
    /// with the mail type, if not used correctly this can write Utf8 to
    /// an Ascii Mail, which is incorrect but has to be safe wrt. rust's safety.
    pub fn write_str_unchecked( &mut self, s: &str) -> Result<()> {
        #[cfg(test)]
        { self.trace.push(Token::NowUnchecked) }
        self.internal_write_str(s)
    }

    /// finishes the writing of a header
    ///
    /// It makes sure the header ends in "\r\n".
    /// If the header ends in a orphan '\r' this
    /// method will just "use" it for the "\r\n".
    ///
    /// If the header ends in a CRLF/start of buffer
    /// followed by only WS (' ' or '\t' ) the valid
    /// header ending is reached by truncating away
    /// the WS padding. This is needed as "blank" lines
    /// are not allowed.
    ///
    /// # Trace (test build only)
    /// can push 0-1 of `[CRLF, TruncateToCRLF]`
    /// then does push `End`
    pub fn finish(&mut self) {
        self.start_new_line();
        #[cfg(test)]
        { self.trace.push(Token::End) }
        self.reinit();
    }

    /// undoes all writes to the internal buffer
    /// since the last `finish` or `undo_header` or
    /// creation of this handle
    ///
    /// # Trace (test build only)
    /// also removes tokens pushed since the last
    /// `finish` or `undo_header` or creation of
    /// this handle
    ///
    pub fn undo_header(&mut self) {
        self.buffer.truncate(self.header_start_idx);
        #[cfg(test)]
        { self.trace.truncate(self.trace_start_idx); }
        self.reinit();
    }



    //---------------------------------------------------------------------------------------------/
    //-/////////////////////////// methods only using the public iface   /////////////////////////-/

    /// calls mark_fws_pos and then writes a space
    ///
    /// This method exists for convenience.
    ///
    /// Note that it can not fail a you just pushed
    /// a place to breake the line befor writing a space.
    ///
    /// Note that currently soft line breaks will not
    /// collapse whitespaces so if you use `write_fws`
    /// and then the line is broken there it will start
    /// with two spaces (one from `\r\n ` and one which
    /// had been there before).
    pub fn write_fws(&mut self) {
        self.mark_fws_pos();
        let _ = self.write_char(AsciiChar::Space);
    }



    //---------------------------------------------------------------------------------------------/
    //-///////////////////////////          private methods               ////////////////////////-/

    /// this might partial write some data and then fail.
    /// while we could implement a undo option it makes
    /// little sense for the use case the generally aviable
    /// `undo_header` is enough.
    fn internal_write_str(&mut self, s: &str)  -> Result<()>  {
        for ch in s.chars() {
            self.internal_write_char(ch)?
        }
        Ok(())
    }

    /// if the line has at last one non-WS char a new line
    /// will be started by adding `\r\n` if the current line
    /// only consists of WS then a new line will be started by
    /// removing the blank line (not that WS are only ' ' and '\r')
    fn start_new_line(&mut self) {
        if self.line_has_content() {
            #[cfg(test)]
            { self.trace.push(Token::CRLF) }

            self.buffer.push('\r');
            self.buffer.push('\n');
        } else {
            #[cfg(test)]
            { self.trace.push(Token::TruncateToCRLF) }
            // e.g. if we "broke" the line on a tailing space => "\r\n  "
            // this would not be valid so we cut awy the trailing white space
            // be if we have "ab  " we do not want to cut away the trailing
            // whitespace but just add "\r\n"
            self.buffer.truncate(self.line_start_idx);
        }
        self.line_start_idx = self.buffer.len();
        self.content_since_fws = false;
        self.content_before_fws = false;
        self.last_fws_idx = self.line_start_idx;

    }

    fn break_line_on_fws(&mut self) -> bool {
        if self.content_before_fws && self.last_fws_idx > self.line_start_idx {
            self.buffer.insert_str(self.last_fws_idx, "\r\n ");
            self.line_start_idx = self.last_fws_idx + 2;
            // no need last_fws can be < line_start but
            //self.last_fws_idx = self.line_start_idx;
            self.content_before_fws = false;
            // stays the same:
            //self.content_since_fws = self.content_since_fws
            true
        } else {
            false
        }
    }

    fn internal_write_char(&mut self, ch: char) -> Result<()> {
        if ch == '\n' {
            if self.skipped_cr {
                self.start_new_line()
            } else {
                bail!("orphan '\n' in header");
            }
            self.skipped_cr = false;
            return Ok(());
        } else {
            if self.skipped_cr {
                bail!("orphan '\r' in header");
            }
            if ch == '\r' {
                self.skipped_cr = true;
                return Ok(());
            } else {
                self.skipped_cr = false;
            }
        }

        if self.current_line_byte_length() >= LINE_LEN_SOFT_LIMIT {
            if !self.break_line_on_fws() {
                if self.buffer.len() == LINE_LEN_HARD_LIMIT {
                    bail!("breached hard line length limit (998 chars excluding CRLF)")
                }
            }
        }

        self.buffer.push(ch);
        #[cfg(test)]
        {
            //REFACTOR(NLL): just use a `if let`-`else` with NLL's
            let need_new =
                if let Some(&mut Token::Text(ref mut string)) = self.trace.last_mut() {
                    string.push(ch);
                    false
                } else {
                    true
                };
            if need_new {
                let mut string = String::new();
                string.push(ch);
                self.trace.push(Token::Text(string))
            }

        }

        // we can't allow "blank" lines
        if ch != ' ' && ch != '\t' {
            // if there is no fws this is equiv to line_has_content
            // else line_has_content = self.content_befor_fws|self.content_since_fws
            self.content_since_fws = true;
        }
        Ok(())
    }
}

/// A BodyBuf implementation based on a Vec<u8>
///
/// this is mainly used for having a simple
/// BodyBuf implementation for testing.
pub struct VecBodyBuf(pub Vec<u8>);

impl BodyBuffer for VecBodyBuf {
    fn with_slice<FN, R>(&self, func: FN) -> Result<R>
        where FN: FnOnce(&[u8]) -> Result<R>
    {
        func(self.0.as_slice())
    }
}

/// Trait Implemented by mainly by structs representing a mail or
/// a part of it
pub trait Encodable<B: BodyBuffer> {
    fn encode( &self, encoder:  &mut Encoder<B>) -> Result<()>;
}

#[cfg(test)]
pub fn simplify_trace_tokens<I: IntoIterator<Item=Token>>(inp: I) -> Vec<Token> {
    use std::mem;
    use self::Token::*;
    let iter = inp.into_iter()
        .filter(|t| {
            match *t {
                NowChar |
                NowStr |
                NowAText |
                NowUtf8 |
                NowUnchecked => false,
                _ => true
            }
        });
    let (min, _max) = iter.size_hint();
    let mut out =
        if min > 0 { Vec::with_capacity(min) }
        else { Vec::new() };
    let mut textbf = String::new();
    let mut had_text = false;
    for token in iter {
        match token {
            Text(str) => {
                had_text = true;
                textbf.push_str(&*str)
            },
            e => {
                if had_text {
                    let text = mem::replace(&mut textbf, String::new());
                    out.push(Text(text));
                    had_text = false;
                }
                out.push(e);
            }
        }
    }
    if had_text {
        out.push(Text(textbf))
    }
    out
}

#[cfg(test)]
#[macro_export]
macro_rules! ec_test {
    ( $name:ident, $inp:block => $mt:tt => [ $($tokens:tt)* ] ) => (
        #[test]
        fn $name() {
            use $crate::codec::{
                EncodableInHeader,
                EncodeHeaderHandle,
                Token, Encoder,
                VecBodyBuf
            };
            use std::mem;
            use $crate::error::Result;

            let mail_type = {
                let mt_str = stringify!($mt).to_lowercase();
                match mt_str.as_str() {
                    "utf8" |
                    "internationalized"
                        => $crate::grammar::MailType::Internationalized,
                    "ascii"
                        =>  $crate::grammar::MailType::Ascii,
                    "mime8" |
                    "mime8bit" |
                    "mime8bitenabled"
                        => $crate::grammar::MailType::Mime8BitEnabled,
                    other => panic!( "invalid name for mail type: {}", other)
                }
            };

            let mut encoder = Encoder::<VecBodyBuf>::new(mail_type);
            {
                //REFACTOR(catch): use catch block once stable
                let doit = |ec: &mut EncodeHeaderHandle| -> Result<()> {
                    let input = $inp;
                    let to_encode: &EncodableInHeader = &input;
                    to_encode.encode(ec)?;
                    Ok(())
                };
                let mut handle = encoder.encode_header_handle();
                doit(&mut handle).unwrap();
                // we do not want to finish writing as we might
                // test just parts of headers
                mem::forget(handle);
            }
            let mut expected: Vec<Token> = Vec::new();
            ec_test!{ __PRIV_TO_TOKEN_LIST expected $($tokens)* }
            //skip over the NewSection part
            let got = $crate::codec::simplify_trace_tokens(encoder.trace.into_iter().skip(1));
            assert_eq!(got, expected)
        }
    );

    (__PRIV_TO_TOKEN_LIST $col:ident Text $e:expr) => (
        $col.push($crate::codec::Token::Text({$e}.into()));
    );
    (__PRIV_TO_TOKEN_LIST $col:ident $token:ident) => (
        $col.push($crate::codec::Token::$token);
    );
    (__PRIV_TO_TOKEN_LIST $col:ident Text $e:expr, $($other:tt)*) => ({
        ec_test!{ __PRIV_TO_TOKEN_LIST $col Text $e }
        ec_test!{ __PRIV_TO_TOKEN_LIST $col $($other)* }
    });
    (__PRIV_TO_TOKEN_LIST $col:ident $token:ident, $($other:tt)*) => (
        ec_test!{ __PRIV_TO_TOKEN_LIST $col $token }
        ec_test!{ __PRIV_TO_TOKEN_LIST $col $($other)* }
    );
    (__PRIV_TO_TOKEN_LIST $col:ident ) => ();
    //conflict with nom due to it using a crate exposing compiler_error...
//    (__PRIV_TO_TOKEN_LIST $col:ident $($other:tt)*) => (
//        compiler_error!( concat!(
//            "syntax error in token list: ", stringify!($($other:tt)*)
//        ))
//    )
}

#[cfg(test)]
mod test {
    use ascii::{ AsciiChar, AsciiStr};
    use error::*;
    use grammar::MailType;

    use super::Token::*;
    use super::{
        BodyBuffer,
        Section,
        EncodableClosure
    };

    type _Encoder = super::Encoder<VecBody>;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct VecBody {
        data: Vec<u8>
    }

    impl VecBody {
        fn new(unique_part: u8) -> Self {
            let data = (0..unique_part).map(|x| x as u8).collect();
            VecBody { data }
        }
    }


    impl BodyBuffer for VecBody {
        fn with_slice<FN, R>(&self, func: FN) -> Result<R>
            where FN: FnOnce(&[u8]) -> Result<R>
        {
            func(self.data.as_slice())
        }
    }

    mod test_test_utilities {
        use codec::Token::*;
        use super::super::simplify_trace_tokens;

        #[test]
        fn does_simplify_tokens_strip_nows() {
            let inp = vec![
                NowChar,
                Text("h".into()),
                CRLF,
                NowStr,
                Text("y yo".into()),
                CRLF,
                NowUtf8,
                Text(", what's".into()),
                CRLF,
                NowUnchecked,
                Text("up!".into()),
                CRLF,
                NowAText,
                Text("abc".into())
            ];
            let out = simplify_trace_tokens(inp);
            assert_eq!(out, vec![
                Text("h".into()),
                CRLF,
                Text("y yo".into()),
                CRLF,
                Text(", what's".into()),
                CRLF,
                Text("up!".into()),
                CRLF,
                Text("abc".into())
            ])

        }

        #[test]
        fn simplify_does_collapse_text() {
            let inp = vec![
                NowChar,
                Text("h".into()),
                NowStr,
                Text("y yo".into()),
                NowUtf8,
                Text(", what's".into()),
                NowUnchecked,
                Text(" up! ".into()),
                NowAText,
                Text("abc".into())
            ];
            let out = simplify_trace_tokens(inp);
            assert_eq!(out, vec![
                Text("hy yo, what's up! abc".into())
            ]);
        }

        #[test]
        fn simplify_works_with_empty_text() {
            let inp = vec![
                NowStr,
                Text("".into()),
                CRLF,
            ];
            assert_eq!(simplify_trace_tokens(inp), vec![
                Text("".into()),
                CRLF
            ])
        }

        #[test]
        fn simplify_works_with_trailing_empty_text() {
            let inp = vec![
                Text("a".into()),
                CRLF,
                Text("".into()),
            ];
            assert_eq!(simplify_trace_tokens(inp), vec![
                Text("a".into()),
                CRLF,
                Text("".into())
            ])
        }

    }

    mod EncodableInHeader {
        #![allow(non_snake_case)]
        use super::super::*;
        use super::VecBody;
        use self::Token::*;

        #[test]
        fn is_implemented_for_closures() {
            let text = "hy ho";
            let closure = EncodableClosure(move |handle: &mut EncodeHeaderHandle| {
                handle.write_utf8(text)
            });

            let mut encoder = Encoder::<VecBody>::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(closure.encode(&mut handle));
                handle.finish();
            }
            assert_eq!(encoder.trace.as_slice(), &[
                NewSection,
                NowUtf8,
                Text("hy ho".into()),
                CRLF,
                End
            ])
        }
    }


    mod Encoder {
        #![allow(non_snake_case)]
        use super::*;
        use super::{ _Encoder as Encoder };

        #[test]
        fn new_encoder() {
            let encoder = Encoder::new(MailType::Internationalized);
            assert_eq!(encoder.mail_type(), MailType::Internationalized);
        }

        #[test]
        fn writing_bodies() {
            let mut encoder = Encoder::new(MailType::Ascii);
            let body1 = VecBody::new(0);
            encoder.write_body(body1.clone());
            let body2 = VecBody::new(5);
            encoder.write_body(body2.clone());

            let res = encoder
                .into_sections()
                .into_iter()
                .map(|s| match s {
                    Section::Header(..) => panic!("we only added bodies"),
                    Section::MIMEBody(body) => body
                })
                .collect::<Vec<_>>();

            let expected = vec![ body1, body2 ];

            assert_eq!(res, expected);
        }

    }


    mod EncodeHeaderHandle {
        #![allow(non_snake_case)]
        use std::mem;

        use super::*;
        use super::{ _Encoder as Encoder };

        #[test]
        fn undo_does_undo() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_str(AsciiStr::from_ascii("Header-One: 12").unwrap()));
                handle.undo_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from(""));
        }

        #[test]
        fn undo_does_not_undo_to_much() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_str(AsciiStr::from_ascii("Header-One: 12").unwrap()));
                handle.finish();
                assert_ok!(handle.write_str(AsciiStr::from_ascii("ups: sa").unwrap()));
                handle.undo_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12\r\n"));
        }

        #[test]
        fn finish_adds_crlf_if_needed() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_str(AsciiStr::from_ascii("Header-One: 12").unwrap()));
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12\r\n"));
        }

        #[test]
        fn finish_does_not_add_crlf_if_not_needed() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_str(AsciiStr::from_ascii("Header-One: 12\r\n").unwrap()));
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12\r\n"));
        }

        #[test]
        fn finish_does_truncat_if_needed() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_str(AsciiStr::from_ascii("Header-One: 12\r\n   ").unwrap()));
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12\r\n"));
        }


        #[test]
        fn finish_can_handle_fws() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_str(AsciiStr::from_ascii("Header-One: 12 +\r\n 4").unwrap()));
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12 +\r\n 4\r\n"));
        }

        #[test]
        fn finish_only_truncats_if_needed() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_str(
                    AsciiStr::from_ascii("Header-One: 12 +\r\n 4  ").unwrap()));
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12 +\r\n 4  \r\n"));
        }


        #[test]
        fn orphan_lf_error() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_err!(handle.write_str(AsciiStr::from_ascii("H: \na").unwrap()));
                handle.undo_header()
            }
        }
        #[test]
        fn orphan_cr_error() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_err!(handle.write_str(AsciiStr::from_ascii("H: \ra").unwrap()));
                handle.undo_header()
            }
        }

        #[test]
        fn orphan_trailing_lf() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_err!(handle.write_str(AsciiStr::from_ascii("H: a\n").unwrap()));
                handle.undo_header();
            }
        }

        #[test]
        fn orphan_trailing_cr() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_str(AsciiStr::from_ascii("H: a\r").unwrap()));
                //it's fine not to error in the trailing \r case as we want to write
                //a \r\n anyway
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("H: a\r\n"));

        }

        #[test]
        fn break_line_on_fws() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_str(AsciiStr::from_ascii("A23456789:").unwrap()));
                handle.mark_fws_pos();
                assert_ok!(handle.write_str(AsciiStr::from_ascii(concat!(
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789",
                    "12345678XX"
                )).unwrap()));
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(&*last, concat!(
                    "A23456789:\r\n ",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789",
                    "12345678XX\r\n"
                ));
        }

        #[test]
        fn to_long_unbreakable_line() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_str(AsciiStr::from_ascii("A23456789:").unwrap()));
                handle.mark_fws_pos();
                assert_ok!(handle.write_str(AsciiStr::from_ascii(concat!(
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789",
                    "80_3456789",
                    "90_3456789",
                    "00_3456789",
                )).unwrap()));
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(&*last, concat!(
                    "A23456789:\r\n ",
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789",
                    "80_3456789",
                    "90_3456789",
                    "00_3456789\r\n",
                ));
        }

        #[test]
        fn multiple_lines_breaks() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_str(AsciiStr::from_ascii("A23456789:").unwrap()));
                handle.mark_fws_pos();
                assert_ok!(handle.write_str(AsciiStr::from_ascii(concat!(
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789",
                )).unwrap()));
                handle.mark_fws_pos();
                assert_ok!(handle.write_str(AsciiStr::from_ascii(concat!(
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                )).unwrap()));
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(&*last, concat!(
                    "A23456789:\r\n ",
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789\r\n ",
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789\r\n",
                ));
        }

        #[test]
        fn hard_line_limit() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                for x in 0..998 {
                    if let Err(_) = handle.write_char(AsciiChar::X) {
                        panic!("error when writing char nr.: {:?}", x+1)
                    }
                }
                let res = &[
                    handle.write_char(AsciiChar::X).is_err(),
                    handle.write_char(AsciiChar::X).is_err(),
                    handle.write_char(AsciiChar::X).is_err(),
                    handle.write_char(AsciiChar::X).is_err(),
                ];
                assert_eq!(
                    res, &[true, true, true, true]
                );
                handle.undo_header();
            }
        }

        #[test]
        fn write_utf8_fail_on_ascii_mail() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_err!(handle.write_utf8("↓"));
                handle.undo_header();
            }
        }

        #[test]
        fn write_utf8_ascii_string_fail_on_ascii_mail() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_err!(handle.write_utf8("just_ascii"));
                handle.undo_header();
            }
        }

        #[test]
        fn write_utf8_ok_on_internationalized_mail() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_utf8("❤"));
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("❤\r\n"));
        }

        #[test]
        fn try_write_atext_ascii() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.try_write_atext("hoho"));
                assert_err!(handle.try_write_atext("a(b"));
                assert_ok!(handle.try_write_atext(""));
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("hoho\r\n"));
        }

        #[test]
        fn try_write_atext_internationalized() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.try_write_atext("hoho"));
                assert_err!(handle.try_write_atext("a(b"));
                assert_ok!(handle.try_write_atext("❤"));
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("hoho❤\r\n"));
        }

        #[test]
        fn multiple_finish_calls_are_ok() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.try_write_atext("hoho"));
                assert_err!(handle.try_write_atext("a(b"));
                assert_ok!(handle.try_write_atext("❤"));
                handle.finish();
                handle.finish();
                handle.finish();
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("hoho❤\r\n"));
        }

        #[test]
        fn multiple_finish_and_undo_calls() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.try_write_atext("hoho"));
                assert_err!(handle.try_write_atext("a(b"));
                assert_ok!(handle.try_write_atext("❤"));
                handle.undo_header();
                handle.finish();
                handle.undo_header();
                handle.undo_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from(""));
        }

        #[test]
        fn header_body_header() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_utf8("H: yay"));
                handle.finish();
            }
            let body = VecBody::new(3);
            encoder.write_body(body.clone());
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_utf8("❤"));
                handle.finish();
            }
            assert_eq!(encoder.sections.len(), 3);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("❤\r\n"));
            let last = encoder.sections.pop().unwrap().unwrap_body();
            assert_eq!(last, body);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("H: yay\r\n"));
        }



        #[test]
        fn drop_without_write_is_ok() {
            let mut encoder = Encoder::new(MailType::Ascii);
            let handle = encoder.encode_header_handle();
            mem::drop(handle)
        }

        #[test]
        fn drop_after_undo_is_ok() {
            let mut encoder = Encoder::new(MailType::Ascii);
            let mut handle = encoder.encode_header_handle();
            assert_ok!(handle.write_str(AsciiStr::from_ascii("Header-One").unwrap()));
            handle.undo_header();
            mem::drop(handle);
        }

        #[test]
        fn drop_after_finish_is_ok() {
            let mut encoder = Encoder::new(MailType::Ascii);
            let mut handle = encoder.encode_header_handle();
            assert_ok!(handle.write_str(AsciiStr::from_ascii("Header-One: 12").unwrap()));
            handle.finish();
            mem::drop(handle);
        }

        #[should_panic]
        #[test]
        fn drop_unfinished_panics() {
            let mut encoder = Encoder::new(MailType::Ascii);
            let mut handle = encoder.encode_header_handle();
            assert_ok!(handle.write_str(AsciiStr::from_ascii("Header-One:").unwrap()));
            mem::drop(handle);
        }

        #[test]
        fn trace_and_undo() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_utf8("something"));
                handle.mark_fws_pos();
                assert_ok!(handle.write_utf8("<else>"));
                handle.undo_header();
            }
            assert_eq!(encoder.trace.len(), 1);
        }

        #[test]
        fn trace_and_undo_does_do_to_much() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_utf8("H: a"));
                handle.finish();
                assert_ok!(handle.write_utf8("something"));
                handle.mark_fws_pos();
                assert_ok!(handle.write_utf8("<else>"));
                handle.undo_header();
            }
            assert_eq!(encoder.trace, vec![
                NewSection,
                NowUtf8,
                Text("H: a".into()),
                CRLF,
                End
            ]);
        }

        #[test]
        fn trace_traces() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_header_handle();
                assert_ok!(handle.write_str(AsciiStr::from_ascii("Header").unwrap()));
                assert_ok!(handle.write_char(AsciiChar::Colon));
                assert_err!(handle.try_write_atext("a(b)c"));
                assert_ok!(handle.try_write_atext("abc"));
                assert_ok!(handle.write_utf8("❤"));
                assert_ok!(handle.write_str_unchecked("remove me\r\n"));
                assert_ok!(handle.write_utf8("   "));
                handle.finish()
            }
            assert_eq!(encoder.trace, vec![
                NewSection,
                NowStr,
                Text("Header".into()),
                NowChar,
                Text(":".into()),
                NowAText,
                Text("abc".into()),
                NowUtf8,
                Text("❤".into()),
                NowUnchecked,
                Text("remove me".into()),
                CRLF,
                NowUtf8,
                Text("   ".into()),
                TruncateToCRLF,
                End
            ]);
        }
    }

    ec_test! {
        does_ec_test_work,
        {
            use super::EncodeHeaderHandle;
            EncodableClosure(|x: &mut EncodeHeaderHandle| {
                x.write_utf8("hy")
            })
        } => Utf8 => [
            Text "hy"
        ]
    }

    ec_test! {
        does_ec_test_allow_early_return,
        {
            use super::EncodeHeaderHandle;
            // this is just a type system test, if it compiles it can bail
            if false { bail!("if false..."); }
            EncodableClosure(|x: &mut EncodeHeaderHandle| {
                x.write_utf8("hy")
            })
        } => Utf8 => [
            Text "hy"
        ]
    }

    mod trait_object {
        use super::super::*;

        #[derive(Default, PartialEq, Debug)]
        struct TestType(&'static str);

        impl EncodableInHeader for TestType {
            fn encode( &self, encoder:  &mut EncodeHeaderHandle ) -> Result<()> {
                encoder.write_utf8(self.0)
            }
        }

        #[derive(Default, PartialEq, Debug)]
        struct AnotherType(&'static str);

        impl EncodableInHeader for AnotherType {
            fn encode( &self, encoder:  &mut EncodeHeaderHandle ) -> Result<()> {
                encoder.write_utf8(self.0)
            }
        }

        #[test]
        fn is() {
            let tt = TestType::default();
            let erased: &EncodableInHeader = &tt;
            assert_eq!( true, erased.is::<TestType>() );
            assert_eq!( false, erased.is::<AnotherType>());
        }

        #[test]
        fn downcast_ref() {
            let tt = TestType::default();
            let erased: &EncodableInHeader = &tt;
            let res: Option<&TestType> = erased.downcast_ref::<TestType>();
            assert_eq!( Some(&tt), res );
            assert_eq!( None, erased.downcast_ref::<AnotherType>() );
        }

        #[test]
        fn downcast_mut() {
            let mut tt_nr2 = TestType::default();
            let mut tt = TestType::default();
            let erased: &mut EncodableInHeader = &mut tt;
            {
                let res: Option<&mut TestType> = erased.downcast_mut::<TestType>();
                assert_eq!( Some(&mut tt_nr2), res );
            }
            assert_eq!( None, erased.downcast_mut::<AnotherType>() );
        }

        #[test]
        fn downcast() {
            let tt = Box::new( TestType::default() );
            let erased: Box<EncodableInHeader> = tt;
            let erased = assert_err!(erased.downcast::<AnotherType>());
            let _: Box<TestType> = assert_ok!(erased.downcast::<TestType>());
        }
    }
}
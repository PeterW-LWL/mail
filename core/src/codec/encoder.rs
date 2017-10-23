use std::any::{Any, TypeId};
use std::fmt::{self, Debug};
use std::result::{ Result as StdResult };

use soft_ascii_string::{SoftAsciiStr, SoftAsciiChar};

use error::{Result, Error};
use grammar::{is_atext, MailType};

use super::traits::BodyBuffer;

/// as specified in RFC 5322 not including CRLF
const LINE_LEN_SOFT_LIMIT: usize = 78;
/// as specified in RFC 5322 (mail) + RFC 5321 (smtp) not including CRLF
const LINE_LEN_HARD_LIMIT: usize = 998;

// can not be moved to `super::traits` as it depends on the
// EncodeHandle defined here
/// Trait Implemented by "components" used in header field bodies
///
/// This trait can be turned into a trait object allowing runtime
/// genericallity over the "components" if needed.
pub trait EncodableInHeader: Any+Debug {
    fn encode(&self, encoder:  &mut EncodeHandle) -> Result<()>;

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
    where FN: for<'a,'b: 'a> Fn(&'a mut EncodeHandle<'b>) -> Result<()>
{
    fn encode(&self, encoder:  &mut EncodeHandle) -> Result<()> {
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
    String(String),
    BodyPayload(R)
}

impl<R> Section<R>
    where R: BodyBuffer
{
    pub fn unwrap_header(self) -> String {
        if let Section::String(res) = self {
            res
        } else {
            panic!("expected `Section::Header` got `Section::Body`")
        }
    }
    pub fn unwrap_body(self) -> R {
        if let Section::BodyPayload(res) = self {
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
    pub trace: Vec<TraceToken>
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
pub enum TraceToken {
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

    /// returns a new EncodeHandle which contains
    /// a mutable reference to the current string buffer
    ///
    /// # Trace (test build only)
    /// pushes a `NewSection` Token if the the returned
    /// `EncodeHandle` refers to a new empty buffer
    pub fn encode_handle(&mut self ) -> EncodeHandle {
        if let Some(&Section::String(..)) = self.sections.last() {}
        else {
            self.sections.push(Section::String(String::new()));
            #[cfg(test)]
            { self.trace.push(TraceToken::NewSection) }
        }

        if let Some(&mut Section::String(ref mut string)) = self.sections.last_mut() {
            #[cfg(not(test))]
            { EncodeHandle::new(self.mail_type, string) }
            #[cfg(test)]
            { EncodeHandle::new(self.mail_type, string, &mut self.trace) }
        } else {
            //REFACTOR(NLL): with NLL we can combine both if-else blocks not needing unreachable! anymore
            unreachable!("we already made sure the last is Section::Header")
        }
    }

    /// calls the provided function with a EncodeHandle cleaning up afterwards
    ///
    /// After calling `func` with the EncodeHandle following cleanup is performed:
    /// - if `func` returned an error `handle.undo_header()` is called, this won't
    ///   undo anything before a `finish_header()` call but will discard partial
    ///   writes
    /// - if `func` succeeded `handle.finish_header()` is called
    pub fn write_header_line<FN>(&mut self, func: FN) -> Result<()>
        where FN: FnOnce(&mut EncodeHandle) -> Result<()>
    {
        let mut handle  = self.encode_handle();
        match func(&mut handle) {
            Ok(()) => {
                handle.finish_header();
                Ok(())
            },
            Err(e) => {
                handle.undo_header();
                Err(e.chain_err(|| "with_handle finished with an error" ))
            }
        }

    }

    pub fn add_blank_line(&mut self) {
        if let Some(&Section::String(..)) = self.sections.last() {}
            else {
                self.sections.push(Section::String(String::new()));
                #[cfg(test)]
                { self.trace.push(TraceToken::NewSection); }
            }

        if let Some(&mut Section::String(ref mut string)) = self.sections.last_mut() {
            string.push_str("\r\n");
            #[cfg(test)]
            { self.trace.push(TraceToken::BlankLine); }
        } else {
            //REFACTOR(NLL): with NLL we can combine both if-else blocks not needing unreachable! anymore
            unreachable!("we already made sure the last is Section::Header")
        }
    }

    /// adds adds a body payload buffer to the encoder
    /// without validating it, the encoder mainly provides
    /// buffers it is not validating them.
    pub fn add_body(&mut self, body: B) {
        self.sections.push(Section::BodyPayload(body))
    }

    pub fn into_sections(self) -> Vec<Section<B>> {
        self.sections
    }

    //TODO other "write out" methods (iterator over &[u8]?)
    pub fn into_string_lossy(self) -> Result<String> {
        let mut out = String::new();
        for section in self.sections.into_iter() {
            match section {
                Section::String(string) => {
                    out.push_str(&*string)
                },
                Section::BodyPayload(body) => {
                    body.with_slice(|slice| {
                        let string = String::from_utf8_lossy(slice);
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
/// Note any act of writing a header through `EncodeHandle`
/// has to be concluded by either calling `finish_header` or `undo_header`.
/// If not this handle will panic in _test_ builds when being dropped
/// (and the thread is not already panicing) as writes through the handle are directly
/// writes to the underlying buffer which now contains malformed/incomplete
/// data. (Note that this Handle does not own any Drop types so if
/// needed `forget`-ing it won't leak any memory)
///
///
pub struct EncodeHandle<'a> {
    buffer: &'a mut String,
    #[cfg(test)]
    trace: &'a mut Vec<TraceToken>,
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

#[cfg(test)]
impl<'a> Drop for EncodeHandle<'a> {

    fn drop(&mut self) {
        use std::thread;
        if !thread::panicking() &&  self.has_unfinished_parts() {
            // we really should panic as the back buffer i.e. the mail will contain
            // some partially written header which definitely is a bug
            panic!("dropped Handle which partially wrote header to back buffer (use `finish_header` or `discard`)")
        }
    }
}

impl<'inner> EncodeHandle<'inner> {

    #[cfg(not(test))]
    fn new(
        mail_type: MailType,
        buffer: &'inner mut String,
    ) -> Self {
        let start_idx = buffer.len();
        EncodeHandle {
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
        buffer: &'inner mut String,
        trace: &'inner mut Vec<TraceToken>
    ) -> Self {
        let start_idx = buffer.len();
        let trace_start_idx = trace.len();
        EncodeHandle {
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
        { self.trace.push(TraceToken::MarkFWS) }
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
    pub fn write_char(&mut self, ch: SoftAsciiChar) -> Result<()>  {
        #[cfg(test)]
        { self.trace.push(TraceToken::NowChar) }
        self.internal_write_char(ch.into())
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
    pub fn write_str(&mut self, s: &SoftAsciiStr)  -> Result<()>  {
        #[cfg(test)]
        { self.trace.push(TraceToken::NowStr) }
        self.internal_write_str(s.as_str())
    }


    /// writes a utf8 str into a buffer for an internationalized mail
    ///
    /// # Error (ConditionalWriteResult)
    /// - fails with `ConditionFailure` if the underlying MailType
    ///    is not Internationalized
    /// - fails with `GeneralFailure` if the hard line length limit is reached
    /// - or if the buffer would contain a orphan '\r' or '\n' after the write
    ///
    /// Note that in case of an error part of the content might already
    /// have been written to the buffer, therefore it is recommended
    /// to call `undo_header` after an error (especially if the
    /// handle is droped after this!)
    ///
    /// # Trace (test build only)
    /// does push `NowUtf8` and then can push `Text`,`CRLF`
    pub fn write_if_utf8<'short>(&'short mut self, s: &str)
        -> ConditionalWriteResult<'short, 'inner>
    {
        if self.mail_type().is_internationalized() {
            #[cfg(test)]
            { self.trace.push(TraceToken::NowUtf8) }
            self.internal_write_str(s).into()
        } else {
            ConditionalWriteResult::ConditionFailure(self)
        }
    }

    pub fn write_utf8(&mut self, s: &str) -> Result<()> {
        if self.mail_type().is_internationalized() {
            #[cfg(test)]
            { self.trace.push(TraceToken::NowUtf8) }
            self.internal_write_str(s).into()
        } else {
            bail!("writing utf8 into non internationalized headers")
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
    /// # Error (ConditionalWriteResult)
    /// - fails with `ConditionFailure` if the text is not valid atext,
    ///   this indicirectly also includes the utf8/Internationalization check
    ///   as the `atext` grammar differs between normal and internationalized
    ///   mail.
    /// - fails with `GeneralGailure` if the hard line length limit is reached and
    ///   the line can't be broken with soft line breaks
    /// - or if buffer would contain a orphan '\r' or '\n' after the write
    ///   (excluding a tailing `'\r'` as it is still valid if followed by an
    ///    `'\n'`)
    ///
    /// Note that in case of an error part of the content might already
    /// have been written to the buffer, therefore it is recommended
    /// to call `undo_header` after an error (especially if the
    /// handle is doped after this!)
    ///
    /// # Trace (test build only)
    /// does push `NowAText` and then can push `Text`
    ///
    pub fn write_if_atext<'short>(&'short mut self, s: &str)
        -> ConditionalWriteResult<'short, 'inner>
    {
        if s.chars().all( |ch| is_atext( ch, self.mail_type() ) ) {
            #[cfg(test)]
            { self.trace.push(TraceToken::NowAText) }
            // the ascii or not aspect is already converted by `is_atext`
            self.internal_write_str(s).into()
        } else {
            ConditionalWriteResult::ConditionFailure(self)
        }

    }

    //TODO remove once SoftAsciiString lands
    /// writes a string to the encoder without checking if it is compatible
    /// with the mail type, if not used correctly this can write Utf8 to
    /// an Ascii Mail, which is incorrect but has to be safe wrt. rust's safety.
    pub fn write_str_unchecked( &mut self, s: &str) -> Result<()> {
        #[cfg(test)]
        { self.trace.push(TraceToken::NowUnchecked) }
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
    /// - can push 0-1 of `[CRLF, TruncateToCRLF]`
    /// - then does push `End`
    /// - calling `funish_current()` multiple times in a row
    ///   will not generate multiple `End` tokens, just one
    pub fn finish_header(&mut self) {
        self.start_new_line();
        #[cfg(test)]
        { if let Some(&TraceToken::End) = self.trace.last() {}
            else { self.trace.push(TraceToken::End) } }
        self.reinit();
    }

    /// undoes all writes to the internal buffer
    /// since the last `finish_header` or `undo_header` or
    /// creation of this handle
    ///
    /// # Trace (test build only)
    /// also removes tokens pushed since the last
    /// `finish_header` or `undo_header` or creation of
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
        let _ = self.write_char(SoftAsciiChar::from_char_unchecked(' '));
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
            { self.trace.push(TraceToken::CRLF) }

            self.buffer.push('\r');
            self.buffer.push('\n');
        } else {
            #[cfg(test)]
            {
                if self.buffer.len() > self.line_start_idx {
                    self.trace.push(TraceToken::TruncateToCRLF);
                }
            }
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
                if let Some(&mut TraceToken::Text(ref mut string)) = self.trace.last_mut() {
                    string.push(ch);
                    false
                } else {
                    true
                };
            if need_new {
                let mut string = String::new();
                string.push(ch);
                self.trace.push(TraceToken::Text(string))
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

pub enum ConditionalWriteResult<'a, 'b: 'a> {
    Ok,
    ConditionFailure(&'a mut EncodeHandle<'b>),
    GeneralFailure(Error)
}

impl<'a, 'b: 'a> From<Result<()>> for ConditionalWriteResult<'a, 'b> {
    fn from(v: Result<()>) -> Self {
        match v {
            Ok(()) => ConditionalWriteResult::Ok,
            Err(e) => ConditionalWriteResult::GeneralFailure(e)
        }
    }
}

impl<'a, 'b: 'a> ConditionalWriteResult<'a, 'b> {

    #[inline]
    pub fn handle_condition_failure<FN>(self, func: FN) -> Result<()>
        where FN: FnOnce(&mut EncodeHandle) -> Result<()>
    {
        use self::ConditionalWriteResult as CWR;

        match self {
            CWR::Ok => Ok(()),
            CWR::ConditionFailure(handle) => {
                func(handle)
            },
            CWR::GeneralFailure(err) => Err(err)
        }
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
pub fn simplify_trace_tokens<I: IntoIterator<Item=TraceToken>>(inp: I) -> Vec<TraceToken> {
    use std::mem;
    use self::TraceToken::*;
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
            #![allow(unused_mut)]
            use $crate::codec::{
                EncodableInHeader,
                EncodeHandle,
                Encoder,
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
                let doit = |ec: &mut EncodeHandle| -> Result<()> {
                    let input = $inp;
                    let to_encode: &EncodableInHeader = &input;
                    to_encode.encode(ec)?;
                    Ok(())
                };
                let mut handle = encoder.encode_handle();
                doit(&mut handle).unwrap();
                // we do not want to finish writing as we might
                // test just parts of headers
                mem::forget(handle);
            }
            let mut expected: Vec<$crate::codec::TraceToken> = Vec::new();
            ec_test!{ __PRIV_TO_TOKEN_LIST expected $($tokens)* }
            //skip over the NewSection part
            let got = $crate::codec::simplify_trace_tokens(encoder.trace.into_iter().skip(1));
            assert_eq!(got, expected)
        }
    );

    (__PRIV_TO_TOKEN_LIST $col:ident Text $e:expr) => (
        $col.push($crate::codec::TraceToken::Text({$e}.into()));
    );
    (__PRIV_TO_TOKEN_LIST $col:ident $token:ident) => (
        $col.push($crate::codec::TraceToken::$token);
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
    use soft_ascii_string::{ SoftAsciiChar, SoftAsciiStr};
    use error::*;
    use grammar::MailType;

    use super::TraceToken::*;
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
        use codec::TraceToken::*;
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
        use self::TraceToken::*;

        #[test]
        fn is_implemented_for_closures() {
            let text = "hy ho";
            let closure = EncodableClosure(move |handle: &mut EncodeHandle| {
                handle.write_utf8(text)
            });

            let mut encoder = Encoder::<VecBody>::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(closure.encode(&mut handle));
                handle.finish_header();
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
            encoder.add_body(body1.clone());
            let body2 = VecBody::new(5);
            encoder.add_body(body2.clone());

            let res = encoder
                .into_sections()
                .into_iter()
                .map(|s| match s {
                    Section::String(..) => panic!("we only added bodies"),
                    Section::BodyPayload(body) => body
                })
                .collect::<Vec<_>>();

            let expected = vec![ body1, body2 ];

            assert_eq!(res, expected);
        }

    }


    mod EncodeHandle {
        #![allow(non_snake_case)]
        use std::mem;

        use super::*;
        use super::{ _Encoder as Encoder };

        #[test]
        fn undo_does_undo() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(
                    handle.write_str(SoftAsciiStr::from_str_unchecked("Header-One: 12")));
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
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One: 12").unwrap()));
                handle.finish_header();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("ups: sa").unwrap()));
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
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One: 12").unwrap()));
                handle.finish_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12\r\n"));
        }

        #[test]
        fn finish_does_not_add_crlf_if_not_needed() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One: 12\r\n").unwrap()));
                handle.finish_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12\r\n"));
        }

        #[test]
        fn finish_does_truncat_if_needed() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One: 12\r\n   ").unwrap()));
                handle.finish_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12\r\n"));
        }


        #[test]
        fn finish_can_handle_fws() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One: 12 +\r\n 4").unwrap()));
                handle.finish_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12 +\r\n 4\r\n"));
        }

        #[test]
        fn finish_only_truncats_if_needed() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_str(
                    SoftAsciiStr::from_str("Header-One: 12 +\r\n 4  ").unwrap()));
                handle.finish_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12 +\r\n 4  \r\n"));
        }


        #[test]
        fn orphan_lf_error() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                assert_err!(handle.write_str(SoftAsciiStr::from_str("H: \na").unwrap()));
                handle.undo_header()
            }
        }
        #[test]
        fn orphan_cr_error() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                assert_err!(handle.write_str(SoftAsciiStr::from_str("H: \ra").unwrap()));
                handle.undo_header()
            }
        }

        #[test]
        fn orphan_trailing_lf() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                assert_err!(handle.write_str(SoftAsciiStr::from_str("H: a\n").unwrap()));
                handle.undo_header();
            }
        }

        #[test]
        fn orphan_trailing_cr() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("H: a\r").unwrap()));
                //it's fine not to error in the trailing \r case as we want to write
                //a \r\n anyway
                handle.finish_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("H: a\r\n"));

        }

        #[test]
        fn break_line_on_fws() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("A23456789:").unwrap()));
                handle.mark_fws_pos();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str(concat!(
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789",
                    "12345678XX"
                )).unwrap()));
                handle.finish_header();
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
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("A23456789:").unwrap()));
                handle.mark_fws_pos();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str(concat!(
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
                handle.finish_header();
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
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("A23456789:").unwrap()));
                handle.mark_fws_pos();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str(concat!(
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789",
                )).unwrap()));
                handle.mark_fws_pos();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str(concat!(
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                )).unwrap()));
                handle.finish_header();
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
                let mut handle = encoder.encode_handle();
                for x in 0..998 {
                    if let Err(_) = handle.write_char(SoftAsciiChar::from_char_unchecked('X')) {
                        panic!("error when writing char nr.: {:?}", x+1)
                    }
                }
                let res = &[
                    handle.write_char(SoftAsciiChar::from_char_unchecked('X')).is_err(),
                    handle.write_char(SoftAsciiChar::from_char_unchecked('X')).is_err(),
                    handle.write_char(SoftAsciiChar::from_char_unchecked('X')).is_err(),
                    handle.write_char(SoftAsciiChar::from_char_unchecked('X')).is_err(),
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
                let mut handle = encoder.encode_handle();
                assert_err!(handle.write_utf8("↓"));
                handle.undo_header();
            }
        }

        #[test]
        fn write_utf8_ascii_string_fail_on_ascii_mail() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                assert_err!(handle.write_utf8("just_ascii"));
                handle.undo_header();
            }
        }

        #[test]
        fn write_utf8_ok_on_internationalized_mail() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_utf8("❤"));
                handle.finish_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("❤\r\n"));
        }

        #[test]
        fn try_write_atext_ascii() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_if_atext("hoho")
                    .handle_condition_failure(|_|panic!("no condition failur expected")));
                let mut had_cond_failure = false;
                assert_ok!(handle.write_if_atext("a(b")
                    .handle_condition_failure(|_| {had_cond_failure=true; Ok(())}));
                assert!(had_cond_failure);
                assert_ok!(handle.write_if_atext("")
                    .handle_condition_failure(|_|panic!("no condition failur expected")));
                handle.finish_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("hoho\r\n"));
        }

        #[test]
        fn try_write_atext_internationalized() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_if_atext("hoho")
                    .handle_condition_failure(|_|panic!("no condition failur expected")));
                let mut had_cond_failure = false;
                assert_ok!(handle.write_if_atext("a(b")
                    .handle_condition_failure(|_| {had_cond_failure=true; Ok(())}));
                assert!(had_cond_failure);
                assert_ok!(handle.write_if_atext("❤")
                    .handle_condition_failure(|_|panic!("no condition failur expected")));
                handle.finish_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("hoho❤\r\n"));
        }

        #[test]
        fn multiple_finish_calls_are_ok() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_if_atext("hoho")
                    .handle_condition_failure(|_|panic!("no condition failur expected")));
                let mut had_cond_failure = false;
                assert_ok!(handle.write_if_atext("a(b")
                    .handle_condition_failure(|_| {had_cond_failure=true; Ok(())}));
                assert!(had_cond_failure);
                assert_ok!(handle.write_if_atext("❤")
                    .handle_condition_failure(|_|panic!("no condition failur expected")));
                handle.finish_header();
                handle.finish_header();
                handle.finish_header();
                handle.finish_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("hoho❤\r\n"));
        }

        #[test]
        fn multiple_finish_and_undo_calls() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_if_atext("hoho")
                    .handle_condition_failure(|_|panic!("no condition failur expected")));
                handle.undo_header();
                handle.finish_header();
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
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_utf8("H: yay"));
                handle.finish_header();
            }
            let body = VecBody::new(3);
            encoder.add_body(body.clone());
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_utf8("❤"));
                handle.finish_header();
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
        fn has_unfinished_parts() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_utf8("Abc:"));
                assert!(handle.has_unfinished_parts());
                handle.undo_header();
                assert_not!(handle.has_unfinished_parts());
                assert_ok!(handle.write_utf8("Abc: c"));
                assert!(handle.has_unfinished_parts());
                handle.finish_header();
                assert_not!(handle.has_unfinished_parts());
            }
        }

        #[test]
        fn drop_without_write_is_ok() {
            let mut encoder = Encoder::new(MailType::Ascii);
            let handle = encoder.encode_handle();
            mem::drop(handle)
        }

        #[test]
        fn drop_after_undo_is_ok() {
            let mut encoder = Encoder::new(MailType::Ascii);
            let mut handle = encoder.encode_handle();
            assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One").unwrap()));
            handle.undo_header();
            mem::drop(handle);
        }

        #[test]
        fn drop_after_finish_is_ok() {
            let mut encoder = Encoder::new(MailType::Ascii);
            let mut handle = encoder.encode_handle();
            assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One: 12").unwrap()));
            handle.finish_header();
            mem::drop(handle);
        }

        #[should_panic]
        #[test]
        fn drop_unfinished_panics() {
            let mut encoder = Encoder::new(MailType::Ascii);
            let mut handle = encoder.encode_handle();
            assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One:").unwrap()));
            mem::drop(handle);
        }

        #[test]
        fn trace_and_undo() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut handle = encoder.encode_handle();
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
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_utf8("H: a"));
                handle.finish_header();
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
                let mut handle = encoder.encode_handle();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header").unwrap()));
                assert_ok!(handle.write_char(SoftAsciiChar::from_char_unchecked(':')));
                let mut had_cond_failure = false;
                assert_ok!(handle.write_if_atext("a(b)c")
                    .handle_condition_failure(|_|{had_cond_failure=true; Ok(())}));
                assert_ok!(handle.write_if_atext("abc")
                    .handle_condition_failure(|_|panic!("unexpected cond failure")));
                assert_ok!(handle.write_utf8("❤"));
                assert_ok!(handle.write_str_unchecked("remove me\r\n"));
                assert_ok!(handle.write_utf8("   "));
                handle.finish_header()
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

        #[test]
        fn with_handle_on_error() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            let res = encoder.write_header_line(|hdl| {
                hdl.write_utf8("some partial writes")?;
                bail!("error ;=)")
            });
            assert_err!(res);
            assert_eq!(encoder.trace, vec![NewSection]);
            assert_eq!(encoder.sections, vec![Section::String("".into())]);
        }

        #[test]
        fn with_handle_partial_writes() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            let res = encoder.write_header_line(|hdl| {
                hdl.write_utf8("X-A: 12")
            });
            assert_ok!(res);
            assert_eq!(encoder.trace, vec![
                NewSection,
                NowUtf8,
                Text("X-A: 12".into()),
                CRLF,
                End
            ]);
            assert_eq!(encoder.sections, vec![
                Section::String("X-A: 12\r\n".into())
            ])
        }

        #[test]
        fn with_handle_ok() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            let res = encoder.write_header_line(|hdl| {
                hdl.write_utf8("X-A: 12")?;
                hdl.finish_header();
                Ok(())
            });
            assert_ok!(res);
            assert_eq!(encoder.trace, vec![
                NewSection,
                NowUtf8,
                Text("X-A: 12".into()),
                CRLF,
                End,
            ]);
            assert_eq!(encoder.sections, vec![
                Section::String("X-A: 12\r\n".into())
            ])
        }

        #[test]
        fn douple_write_fws() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            let res = encoder.write_header_line(|hdl| {
                hdl.write_fws();
                hdl.write_fws();
                Ok(())
            });
            assert_ok!(res);
            assert_eq!(encoder.trace, vec![
                NewSection,
                MarkFWS, NowChar, Text(" ".to_owned()),
                MarkFWS, NowChar, Text(" ".to_owned()),
                TruncateToCRLF,
                End
            ]);
            assert_eq!(encoder.sections, vec![
                Section::String("".to_owned())
            ])
        }
        #[test]
        fn douple_write_fws_then_long_line() {
            let long_line = concat!(
                "10_3456789",
                "20_3456789",
                "30_3456789",
                "40_3456789",
                "50_3456789",
                "60_3456789",
                "70_3456789",
                "80_3456789",
            );
            let mut encoder = Encoder::new(MailType::Internationalized);
            let res = encoder.write_header_line(|hdl| {
                hdl.write_fws();
                hdl.write_fws();
                hdl.write_utf8(long_line)?;
                Ok(())
            });
            assert_ok!(res);
            assert_eq!(encoder.trace, vec![
                NewSection,
                MarkFWS, NowChar, Text(" ".to_owned()),
                MarkFWS, NowChar, Text(" ".to_owned()),
                NowUtf8, Text(long_line.to_owned()),
                CRLF,
                End
            ]);
            assert_eq!(encoder.sections, vec![
                Section::String(format!("  {}\r\n", long_line))
            ])
        }
    }

    ec_test! {
        does_ec_test_work,
        {
            use super::EncodeHandle;
            EncodableClosure(|x: &mut EncodeHandle| {
                x.write_utf8("hy")
            })
        } => Utf8 => [
            Text "hy"
        ]
    }

    ec_test! {
        does_ec_test_allow_early_return,
        {
            use super::EncodeHandle;
            // this is just a type system test, if it compiles it can bail
            if false { bail!("if false..."); }
            EncodableClosure(|x: &mut EncodeHandle| {
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
            fn encode(&self, encoder:  &mut EncodeHandle) -> Result<()> {
                encoder.write_utf8(self.0)
            }
        }

        #[derive(Default, PartialEq, Debug)]
        struct AnotherType(&'static str);

        impl EncodableInHeader for AnotherType {
            fn encode(&self, encoder:  &mut EncodeHandle) -> Result<()> {
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
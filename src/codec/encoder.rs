use std::any::{Any, TypeId};
use std::fmt::Debug;

use ascii::{AsciiStr, AsciiChar};

use error::Result;
use grammar::{is_atext, MailType};

/// as specified in RFC xxxx(822?) not including CRLF
const LINE_LEN_SOFT_LIMIT: usize = 78;
/// as specified in RFC xxxx(822?) not including CRLF
const LINE_LEN_HARD_LIMIT: usize = 998;

#[cfg(test)]
use self::trace_tools::Token;
//IDEA(1):
// how to still have nice tests?
// 1. store a vector of tokens , but well that's unessesary overhead
// so which part of the test thing do we still need?
// => the part about FWS,OptFWS
//
// ==> Store not just the last FWS/OptFWS but all
// ==> Store if it was an FWS or an OptFWS

//IDEA(2):
// seperate header writes and body writes
// header writes are at last string
// body writes should be string but well we don't have a gurantee here in the type system
//==>
// buffer: Vec<Kind>
// enum Kind { HeaderOut(String), BodyOut(Vec<u8>) }
//
// if we do not check Bodies at all (here) why not have some reference there instead of Vec<u8>?
// BodyOut(dyn BodyRef) ??
//
// and we do not support the raw data extensions from SMPTUTF8 so
// maybe use a String buffer for bodies too
//

//CONSIDER 8BITMIME:
//  - allows us to e.g. have "text/plain; charset=utf8" in non-internationalized mails
//  - does not allows usage of internationalized mail addresses or utf8 in mim header
//  - but in the mime body
//  - especially `text/html; charset=utf8` is of interest
//  - so it is "just" relevant for the MIME bodies
//  - be we should support it
//  - through some gateways might mess it up, but most support it just fine
//  - so make it an option alla:
//          only transfer encode `text/plain` IF option is set AND mime8bit is supported

//==> write an header only encoder
//==> write and mime body sink
//==> write a buffer combining it

//use on all components
pub trait EncodableInHeader: Any+Debug {
    fn encode(&self, encoder:  &mut EncodeHeaderHandle) -> Result<()>;

    #[doc(hidden)]
    fn type_id( &self ) -> TypeId {
        TypeId::of::<Self>()
    }
}

//use on EncodableMail/Mail/MailPart, maybe
pub trait Encodable {
    //TODO figure out the body buffer zero-copy aspect
    //can be generic as only EncodableInHeader has trait objects
    fn encode<R: BodyBuffer>( &self, encoder:  &mut Encoder<R>) -> Result<()>;
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrphanMode {
    Remove,
    Complete
}

impl Default for OrphanMode {
    fn default() -> Self {
        OrphanMode::Complete
    }
}

//TODO implement BodyBuffer for Resource removing one pointless copy
pub trait BodyBuffer {
    // by making it take a closure it allows us to implement
    // this on all kind of buffers, including such which need
    // handled references like lock guards (which deref to
    // &[u8] but have to be kept alive so are not usable for
    // as_slice which just returns a slice)
    fn with_slice<FN, R>(&self, func: FN) -> Result<R>
        where FN: FnOnce(&[u8]) -> Result<R>;
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

//TODO implement a iter_slices(&self) -> impl Iterator<Item=&[u8]> for
// efficiently copying a encoder result into a bytevector
pub struct Encoder<R: BodyBuffer> {
    mail_type: MailType,
    orphan_mode: OrphanMode,
    sections: Vec<Section<R>>,
    #[cfg(test)]
    pub trace: Vec<Token>
}

#[cfg(test)]
pub mod trace_tools {
    #[derive(Debug, Clone, Eq, PartialEq, Hash)]
    pub enum Token {
        MarkFWS,
        CRLF,
        TruncateToCRLF,
        OrphanCompleted,
        OrphanRemoved,
        Text(String),
        NowChar,
        NowStr,
        NowAText,
        NowUtf8,
        NowUnchecked,
        End
    }
}

impl<B: BodyBuffer> Encoder<B> {

    pub fn new(mail_type: MailType) -> Self {
        Self::new_with_orphan_mode(mail_type, Default::default())
    }

    pub fn new_with_orphan_mode(mail_type: MailType, orphan_mode: OrphanMode) -> Self {
        Encoder {
            mail_type,
            orphan_mode,
            sections: Default::default(),
            #[cfg(test)]
            trace: Vec::new()
        }
    }

    pub fn mail_type( &self ) -> MailType {
        self.mail_type
    }

    pub fn orphan_mode(&self) -> OrphanMode {
        self.orphan_mode
    }

    pub fn encode_header( &mut self ) -> EncodeHeaderHandle {
        if let Some(&Section::Header(..)) = self.sections.last() {}
        else {
            self.sections.push(Section::Header(String::new()))
        }

        if let Some(&mut Section::Header(ref mut string)) = self.sections.last_mut() {
            #[cfg(not(test))]
            { EncodeHeaderHandle::new(self.mail_type, self.orphan_mode, string) }
            #[cfg(test)]
            { EncodeHeaderHandle::new(self.mail_type, self.orphan_mode, string, &mut self.trace) }
        } else {
            //REFACTOR(NLL): with NLL we can combine both if-else blocks not needing unreachable! anymore
            unreachable!("we already made sure the last is Section::Header")
        }
    }

    pub fn write_body( &mut self, body: B) {
        self.sections.push(Section::MIMEBody(body))
    }

    pub fn into_sections(self) -> Vec<Section<B>> {
        self.sections
    }
}


pub struct EncodeHeaderHandle<'a> {
    buffer: &'a mut String,
    #[cfg(test)]
    trace: &'a mut Vec<Token>,
    mail_type: MailType,
    orphan_mode: OrphanMode,
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
        if !thread::panicking() && self.buffer.len() != self.header_start_idx {
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
        orphan_mode: OrphanMode,
        buffer: &'a mut String,
    ) -> Self {
        let start_idx = buffer.len();
        EncodeHeaderHandle {
            buffer,
            mail_type,
            orphan_mode,
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
        orphan_mode: OrphanMode,
        buffer: &'a mut String,
        trace: &'a mut Vec<Token>
    ) -> Self {
        let start_idx = buffer.len();
        let trace_start_idx = trace.len();
        EncodeHeaderHandle {
            buffer,
            trace,
            mail_type,
            orphan_mode,
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

    pub fn mark_fws_pos(&mut self) {
        #[cfg(test)]
        { self.trace.push(Token::MarkFWS) }
        self.content_before_fws |= self.content_since_fws;
        self.content_since_fws = false;
        self.last_fws_idx = self.buffer.len()
    }

    pub fn write_char(&mut self, ch: AsciiChar) -> Result<()>  {
        #[cfg(test)]
        { self.trace.push(Token::NowChar) }
        self.internal_write_char(ch.as_char())
    }

    pub fn write_str(&mut self, s: &AsciiStr)  -> Result<()>  {
        #[cfg(test)]
        { self.trace.push(Token::NowStr) }
        self.internal_write_str(s.as_str())
    }


    pub fn write_utf8(&mut self, s: &str) -> Result<()> {
        if self.mail_type().is_internationalized() {
            #[cfg(test)]
            { self.trace.push(Token::NowUtf8) }
            self.internal_write_str(s)
        } else {
            bail!( "can not write utf8 into Ascii mail" )
        }
    }

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
    pub fn finish(&mut self) {
        // if we have a tailing '\r' and would handle it
        // it would not change anything as in the end
        // only a "\r\n" will added ( e.g. '\r' become
        // "\r\n" => start_new_line will trim it to the
        // buffers len, '\r' becomes "" start_new_line
        // will add "\r\n", etc. wrt. blank lines before
        // the '\r')
        self.start_new_line();
        #[cfg(test)]
        { self.trace.push(Token::End) }
        self.reinit();
    }

    /// undoes all writes to the internal buffer
    /// since the last `finish` call or the
    /// creation of this handle if there hasn't been
    /// a `finish` call before
    pub fn undo_header(&mut self) {
        self.buffer.truncate(self.header_start_idx);
        #[cfg(test)]
        { self.trace.truncate(self.trace_start_idx); }
        self.reinit();
    }



    //---------------------------------------------------------------------------------------------/
    //-/////////////////////////// methods only using the public iface   /////////////////////////-/

    pub fn write_fws(&mut self) {
        self.mark_fws_pos();
        // this cant fail due to line limit as you just
        // stated that the line can be cut the character
        // before this write_char call
        let _ = self.write_char(AsciiChar::Space);
    }



    //---------------------------------------------------------------------------------------------/
    //-///////////////////////////          private methods               ////////////////////////-/

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


    fn handle_orphan(&mut self) {
        match self.orphan_mode {
            OrphanMode::Complete => {
                #[cfg(test)]
                { self.trace.push(Token::OrphanCompleted) }
                self.start_new_line();
            },
            OrphanMode::Remove => {
                #[cfg(test)]
                { self.trace.push(Token::OrphanRemoved) }
            }
        }
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
                self.handle_orphan();
            }
            self.skipped_cr = false;
            return Ok(());
        } else {
            if self.skipped_cr {
                self.handle_orphan()
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


#[cfg(test)]
mod test {
    use ascii::{ AsciiChar, AsciiStr};
    use error::*;
    use grammar::MailType;

    use super::trace_tools::Token::*;
    use super::{
        BodyBuffer,
        Section
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

    mod OrphanMode {
        #![allow(non_snake_case)]
        use super::*;
        use super::super::OrphanMode;

        #[test]
        fn default_is_complete() {
            assert_eq!(OrphanMode::default(), OrphanMode::Complete);
        }
    }

    mod Encoder {
        #![allow(non_snake_case)]
        use std::default::Default;
        use super::*;
        use super::{ _Encoder as Encoder };
        use super::super::{ OrphanMode };

        #[test]
        fn new_encoder() {
            let encoder = Encoder::new(MailType::Internationalized);
            assert_eq!(encoder.mail_type(), MailType::Internationalized);
            assert_eq!(encoder.orphan_mode(), OrphanMode::default());
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
        use super::super::{ EncodeHeaderHandle, OrphanMode };

        #[test]
        fn undo_does_undo() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("Header-One: 12").unwrap()));
                henc.undo_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from(""));
        }

        #[test]
        fn undo_does_not_undo_to_much() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("Header-One: 12").unwrap()));
                henc.finish();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("ups: sa").unwrap()));
                henc.undo_header();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12\r\n"));
        }

        #[test]
        fn finish_adds_crlf_if_needed() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("Header-One: 12").unwrap()));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12\r\n"));
        }

        #[test]
        fn finish_does_not_add_crlf_if_not_needed() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("Header-One: 12\r\n").unwrap()));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12\r\n"));
        }

        #[test]
        fn finish_does_truncat_if_needed() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("Header-One: 12\r\n   ").unwrap()));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12\r\n"));
        }


        #[test]
        fn finish_can_handle_fws() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("Header-One: 12 +\r\n 4").unwrap()));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12 +\r\n 4\r\n"));
        }

        #[test]
        fn finish_only_truncats_if_needed() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(
                    AsciiStr::from_ascii("Header-One: 12 +\r\n 4  ").unwrap()));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("Header-One: 12 +\r\n 4  \r\n"));
        }


        #[test]
        fn orphan_correction_lf_complete() {
            let mut encoder = Encoder::new_with_orphan_mode(MailType::Ascii, OrphanMode::Complete);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("H: \n a").unwrap()));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("H: \r\n a\r\n"));
        }
        #[test]
        fn orphan_correction_lf_remove() {
            let mut encoder = Encoder::new_with_orphan_mode(MailType::Ascii, OrphanMode::Remove);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("H: \n a").unwrap()));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("H:  a\r\n"));
        }

        #[test]
        fn orphan_correction_cr_complete() {
            let mut encoder = Encoder::new_with_orphan_mode(MailType::Ascii, OrphanMode::Complete);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("H: \r a").unwrap()));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("H: \r\n a\r\n"));
        }

        #[test]
        fn orphan_correction_cr_remove() {
            let mut encoder = Encoder::new_with_orphan_mode(MailType::Ascii, OrphanMode::Remove);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("H: \r a").unwrap()));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("H:  a\r\n"));
        }

        #[test]
        fn orphan_correction_trailing_cr() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("H: a\r").unwrap()));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("H: a\r\n"));
        }

        #[test]
        fn break_line_on_fws() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("A23456789:").unwrap()));
                henc.mark_fws_pos();
                assert_ok!(henc.write_str(AsciiStr::from_ascii(concat!(
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789",
                    "12345678XX"
                )).unwrap()));
                henc.finish();
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
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("A23456789:").unwrap()));
                henc.mark_fws_pos();
                assert_ok!(henc.write_str(AsciiStr::from_ascii(concat!(
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
                henc.finish();
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
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("A23456789:").unwrap()));
                henc.mark_fws_pos();
                assert_ok!(henc.write_str(AsciiStr::from_ascii(concat!(
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789",
                )).unwrap()));
                henc.mark_fws_pos();
                assert_ok!(henc.write_str(AsciiStr::from_ascii(concat!(
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                )).unwrap()));
                henc.finish();
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
                let mut henc = encoder.encode_header();
                for x in 0..998 {
                    if let Err(_) = henc.write_char(AsciiChar::X) {
                        panic!("error when writing char nr.: {:?}", x+1)
                    }
                }
                let res = &[
                    henc.write_char(AsciiChar::X).is_err(),
                    henc.write_char(AsciiChar::X).is_err(),
                    henc.write_char(AsciiChar::X).is_err(),
                    henc.write_char(AsciiChar::X).is_err(),
                ];
                assert_eq!(
                    res, &[true, true, true, true]
                );
                henc.undo_header();
            }
        }

        #[test]
        fn write_utf8_fail_on_ascii_mail() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut henc = encoder.encode_header();
                assert_err!(henc.write_utf8("↓"));
                henc.undo_header();
            }
        }

        #[test]
        fn write_utf8_ascii_string_fail_on_ascii_mail() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut henc = encoder.encode_header();
                assert_err!(henc.write_utf8("just_ascii"));
                henc.undo_header();
            }
        }

        #[test]
        fn write_utf8_ok_on_internationalized_mail() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_utf8("❤"));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("❤\r\n"));
        }

        #[test]
        fn try_write_atext_ascii() {
            let mut encoder = Encoder::new(MailType::Ascii);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.try_write_atext("hoho"));
                assert_err!(henc.try_write_atext("a(b"));
                assert_ok!(henc.try_write_atext(""));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("hoho\r\n"));
        }

        #[test]
        fn try_write_atext_internationalized() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.try_write_atext("hoho"));
                assert_err!(henc.try_write_atext("a(b"));
                assert_ok!(henc.try_write_atext("❤"));
                henc.finish();
            }
            assert_eq!(encoder.sections.len(), 1);
            let last = encoder.sections.pop().unwrap().unwrap_header();
            assert_eq!(last, String::from("hoho❤\r\n"));
        }

        #[test]
        fn header_body_header() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_utf8("H: yay"));
                henc.finish();
            }
            let body = VecBody::new(3);
            encoder.write_body(body.clone());
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_utf8("❤"));
                henc.finish();
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
            let henc = encoder.encode_header();
            mem::drop(henc)
        }

        #[test]
        fn drop_after_undo_is_ok() {
            let mut encoder = Encoder::new(MailType::Ascii);
            let mut henc = encoder.encode_header();
            assert_ok!(henc.write_str(AsciiStr::from_ascii("Header-One").unwrap()));
            henc.undo_header();
            mem::drop(henc);
        }

        #[test]
        fn drop_after_finish_is_ok() {
            let mut encoder = Encoder::new(MailType::Ascii);
            let mut henc = encoder.encode_header();
            assert_ok!(henc.write_str(AsciiStr::from_ascii("Header-One: 12").unwrap()));
            henc.finish();
            mem::drop(henc);
        }

        #[should_panic]
        #[test]
        fn drop_unfinished_panics() {
            let mut encoder = Encoder::new(MailType::Ascii);
            let mut henc = encoder.encode_header();
            assert_ok!(henc.write_str(AsciiStr::from_ascii("Header-One:").unwrap()));
            mem::drop(henc);
        }

        #[test]
        fn trace_and_undo() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_utf8("something"));
                henc.mark_fws_pos();
                assert_ok!(henc.write_utf8("<else>"));
                henc.undo_header();
            }
            assert_eq!(encoder.trace.len(), 0);
        }

        #[test]
        fn trace_and_undo_does_do_to_much() {
            let mut encoder = Encoder::new(MailType::Internationalized);
            {
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_utf8("H: a"));
                henc.finish();
                assert_ok!(henc.write_utf8("something"));
                henc.mark_fws_pos();
                assert_ok!(henc.write_utf8("<else>"));
                henc.undo_header();
            }
            assert_eq!(encoder.trace, vec![
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
                let mut henc = encoder.encode_header();
                assert_ok!(henc.write_str(AsciiStr::from_ascii("Header").unwrap()));
                assert_ok!(henc.write_char(AsciiChar::Colon));
                assert_err!(henc.try_write_atext("a(b)c"));
                assert_ok!(henc.try_write_atext("abc"));
                assert_ok!(henc.write_utf8("❤"));
                assert_ok!(henc.write_str_unchecked("remove me\r\n"));
                assert_ok!(henc.write_utf8("   "));
                henc.finish()
            }
            assert_eq!(encoder.trace, vec![
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


}
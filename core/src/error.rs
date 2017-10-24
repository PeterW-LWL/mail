use std::io;
use mime::Mime;
use mime::FromStrError as MimeParsingErr;
use base64;
use quoted_printable;
use idna::uts46::{ Errors as PunyCodeErrors };
use std::fmt::{self, Display};
use std::path::PathBuf;

#[derive(Debug)]
pub struct MultipleErrorsWraper {
    pub errors: Vec<Error>
}

impl From<Vec<Error>> for MultipleErrorsWraper {
    fn from(errors: Vec<Error>) -> MultipleErrorsWraper {
        MultipleErrorsWraper { errors }
    }
}

impl Display for MultipleErrorsWraper {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.debug_list()
            .entries(&self.errors)
            .finish()
    }
}

// we do not wan't dependencies to have to import error_chain
// just to have some of the additional error chaining functions
pub use error_chain::ChainedError;

#[allow(unused_doc_comment)]
error_chain! {

    foreign_links {
        Io( io::Error );
        DecodeBase64(base64::DecodeError);
        DecodeQuotedPrintable(quoted_printable::QuotedPrintableError);
    }


    errors {

        HeaderComponentEncodingFailure {
            description("encoding header component failed")
        }

        PathToFileWithoutFileName(path: PathBuf) {
            description("malformed path for loading a file")
            display("malformed path for loading a file: {:?}", path)
        }

        MultipleErrors(errors: MultipleErrorsWraper) {
            description("multiple errors happened in the same operation")
            display("multiple errors: {}", errors)
        }

        FailedToAddHeader(name: &'static str) {
            description("failed to add a header filed to the header map")
            display("failed to a the field {:?} to the header map", name)
        }
        //mime_error does not impl (std)Error so no chaining possible
        ParsingMime( mime_error: MimeParsingErr ) {
            description( "parsing mime failed" )
            display( "parsing mime failed ({:?})", mime_error )
        }
        /// Certain components might not be encodable under some circumstances.
        /// E.g. they might have non-ascii values and are not encodable into ascii
        ///
        /// a example for this would be a non ascii `local-part` of `addr-spec`
        /// (i.e. the part of a email address befor the `@`)
        NonEncodableComponents( component: &'static str, data: String ) {
            description( "given information can not be encoded into ascii" )
            display( "can not encode the {} component with value {:?}", component, data )
        }

        TriedWriting8BitBytesInto7BitData {
            description(
                "the program tried to write a non ascii string while smtputf8 was not supported" )
        }

        AtLastOneElementIsRequired {
            description( concat!( "for the operation a list with at last one element",
                                  " is required but and empty list was given" ) )
        }

        InvalidHeaderName(name: String) {
            description( "given header name is not valid" )
            display( "{:?} is not a valid header name", name )
        }

        NotMultipartMime( mime: Mime ) {
            description( "expected a multipart mime for a multi part body" )
            display( _self ) -> ( "{}, got: {}", _self.description(), mime )
        }

        MultipartBoundaryMissing {
            description( "multipart boundary is missing" )
        }

        NotSinglepartMime( mime: Mime ) {
            description( "expected a non-multipart mime for a non-multipart body" )
            display( _self ) -> ( "{}, got: {}", _self.description(), mime )
        }

        PunyCodeingDomainFailed( errors: PunyCodeErrors ) {
            description( "using puny code to encode the domain failed" )
        }


        NeedPlainAndOrHtmlMailBody {

        }

        ContentTypeAndBodyIncompatible {
            description( concat!(
                "given content type is incompatible with body,",
                "e.g. using a non multipart mime with a multipart body" ) )
        }

        UnknownTransferEncoding( encoding: String ) {
            description( "the given transfer encoding is not supported" )
            display( "the transfer encoding {:?} is not supported", encoding )
        }

        Invalide7BitValue( byte: u8 ) {
            description( "the byte is not valid in 7bit (content transfer) encoding" )
        }
        Invalide8BitValue( val: u8 ) {
            description( "the byte is not valid in 8bit (content transfer) encoding" )
        }

        Invalide7BitSeq( byte: u8 ) {
            description( "the chars '\\r', '\\n' can only appear as \"\\r\\n\" in 7bit (content transfer) encoding " )
        }
        Invalide8BitSeq( val: u8 ) {
            description( "the chars '\\r', '\\n' can only appear as \"\\r\\n\" in 8bit (content transfer) encoding " )
        }

        BodyFutureResolvedToAnError {
        
        }

        NeedAtLastOneBodyInMultipartMail {

        }

        GeneratingMimeFailed {

        }

        RegisterExtensionsToLate( extension: String ) {
            description( "can not register extensions after Store/Look-Up-Table was generated" )
        }
    }
}

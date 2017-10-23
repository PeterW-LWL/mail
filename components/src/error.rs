use nom::IResult;

quick_error! {
    #[derive(Debug)]
    pub enum ComponentError {

        WSPOnlyPhrase {
            description(
                "can not encode WSP only phrase a phrase is required to contain at last one VCHAR")
        }

        InvalidToken(got: String) {
            description("given input was not a valid token (syntax)")
            display("expected valid token (syntax) got: {:?}", got)
        }

        InvalidContentDisposition(got: String) {
            description(
                "Content-Disposition can either be \"inline\" or \"attachment\""
            )
            display("expected \"inline\" or \"attachment\" got {:?}", got)
        }

        InvalidDomainName(got: String) {
            description("given input is not a valid domain name")
            display("expected a valid domain name, got: {:?}", got)
        }

        InvalidEmail(got: String) {
            description("given input is not a valid Email")
            display("expected a valid Email, got: {:?}", got)
        }

        InvalidMessageId(got: String, nom_parse_output: IResult) {
            description("given input is not a valid MessageId")
            display("expected a valid MessageId, got: {:?}  (nom parsed to: {:?})",
                got, nom_parse_output)
        }

        MailboxListSize0 {
            description("a mailbox list consist of at last one phrase, not 0")
        }

        PhraseListSize0 {
            description("a phrase list consist of at last one phrase, not 0")
        }

        NeedAtLastOneVCHAR(got: String) {
            description("given input did contain 0 VCHAR's but at last 1 was required")
            display("need at last one VCHAR in input got: {:?}", got)
        }
    }
}

#[macro_export]
macro_rules! bail {
    ($ce:expr) => ({
        use $crate::error::ComponentError;
        use $crate::core::error::{ErrorKind, ResultExt};
        let err: ComponentError = $ce;
        return Err(err).chain_err(||ErrorKind::HeaderComponentEncodingFailure)
    });
}
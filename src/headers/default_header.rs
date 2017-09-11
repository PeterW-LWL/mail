//    def_headers! {
//        test_name: validate_header_names,
//        + ContentType, unsafe { "Content-Type" }, components::ContentType
//        + ContentId, unsafe { "Content-ID" }, components::ContentID
//        + Magic, unsafe { "X-Magic" }, x_cmds::Magic
//    }
/// Defines a new header types with given type name, filed name and component
/// Note that the unsafe AsciiStr::from_ascii_unchecked is used with the given
/// header field names.
///
/// Nevertheless a test is created with the given test name which
/// tests if all field names are valide.
#[macro_export]
macro_rules! def_headers {
    (
        test_name: $tn:ident,
        scope: $scope:ident,
        $($multi:tt $name:ident, unsafe { $hname:tt }, $component:ident),+
    ) => (
        $(
            pub struct $name;

            impl $crate::headers::Header for  $name {
                const CAN_APPEAR_MULTIPLE_TIMES: bool = def_headers!(_PRIV_boolify $multi);
                type Component = $scope::$component;

                fn name() -> $crate::headers::HeaderName {
                    let as_str: &'static str = $hname;
                    unsafe { $crate::headers::HeaderName::from_ascii_unchecked( as_str ) }
                }
            }
        )+

        $(
            def_headers!{ _PRIV_impl_marker $multi $name }
        )+

        //TODO warn if header type name and header name diverges
        // (by stringifying the type name and then ziping the
        //  array of type names with header names removing
        //  "-" from the header names and comparing them to
        //  type names)


        #[cfg(test)]
        const HEADER_NAMES: &[ &str ] = &[ $(
            $hname
        ),+ ];

        #[test]
        fn $tn() {
            use std::collections::HashSet;
            use $crate::codec::{ MailEncoder, MailEncodable, MailEncoderImpl };

            let mut name_set = HashSet::new();
            for name in HEADER_NAMES {
                if !name_set.insert(name) {
                    panic!("name appears more than one time in same def_headers macro: {:?}", name);
                }
            }
            fn can_be_trait_object<E: MailEncoder, EN: MailEncodable<E>>( v: Option<&EN> ) {
                let _ = v.map( |en| en as &MailEncodable<E> );
            }
            $(
                can_be_trait_object::<MailEncoderImpl, $scope::$component>( None );
            )+
            for name in HEADER_NAMES {
                let res = $crate::headers::HeaderName::validate_name(
                    $crate::headers::_AsciiStr::from_ascii(name).unwrap()
                );
                if res.is_err() {
                    panic!( "invalid header name: {:?} ({:?})", name, res.unwrap_err() );
                }
            }
        }
    );
    (_PRIV_boolify +) => ({ true });
    (_PRIV_boolify 1) => ({ false });
    (_PRIV_boolify $other:tt) => (
        compiler_error!( "only `1` (for singular) or `+` (for multiple) are valid" )
    );
    ( _PRIV_impl_marker + $name:ident ) => (
        //do nothing here
    );
    ( _PRIV_impl_marker 1 $name:ident ) => (
        impl $crate::headers::SingularHeaderMarker for $name {}
    );
}



use components;
def_headers! {
    test_name: validate_header_names,
    scope: components,
    //RFC 5322:
    1 Date,                    unsafe { "Date"          },  DateTime,
    1 From,                    unsafe { "From"          },  MailboxList,
    1 Sender,                  unsafe { "Sender"        },  Mailbox,
    1 ReplyTo,                 unsafe { "Reply-To"      },  MailboxList,
    1 To,                      unsafe { "To"            },  MailboxList,
    1 Cc,                      unsafe { "Cc"            },  MailboxList,
    1 Bcc,                     unsafe { "Bcc"           },  MailboxList,
    1 MessageId,               unsafe { "Message-Id"    },  MessageID,
    1 InReplyTo,               unsafe { "In-Reply-To"   },  MessageIDList,
    1 References,              unsafe { "References"    },  MessageIDList,
    1 Subject,                 unsafe { "Subject"       },  Unstructured,
    + Comments,                unsafe { "Comments"      },  Unstructured,
    + Keywords,                unsafe { "Keywords"      },  PhraseList,
    + ResentDate,              unsafe { "Resent-Date"   },  DateTime,
    + ResentFrom,              unsafe { "Resent-From"   },  MailboxList,
    + ResentSender,            unsafe { "Resent-Sender" },  Mailbox,
    + ResentTo,                unsafe { "Resent-To"     },  MailboxList,
    + ResentCc,                unsafe { "Resent-Cc"     },  MailboxList,
    + ResentBcc,               unsafe { "Resent-Bcc"    },  OptMailboxList,
    + ResentMsgId,             unsafe { "Resent-Msg-Id" },  MessageID,
    + ReturnPath,              unsafe { "Return-Path"   },  Path,
    + Received,                unsafe { "Received"      },  ReceivedToken,
    //RFC 2045:
    1 ContentType,             unsafe { "Content-Type"              }, Mime,
    1 ContentId,               unsafe { "Content-Id"                }, ContentID,
    1 ContentTransferEncoding, unsafe { "Content-Transfer-Encoding" }, TransferEncoding,
    1 ContentDescription,      unsafe { "Content-Description"       }, Unstructured,
    //RFC 2183:
    1 ContentDisposition,      unsafe { "Content-Disposition"       }, Disposition
}

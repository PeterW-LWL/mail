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
        $($multi:tt $name:ident, unsafe { $hname:tt }, $component:ident, $validator:ident),+
    ) => (
        $(
            pub struct $name;

            impl $crate::headers::Header for  $name {
                const MAX_COUNT_EQ_1: bool = def_headers!(_PRIV_boolify $multi);
                type Component = $scope::$component;

                fn name() -> $crate::headers::HeaderName {
                    let as_str: &'static str = $hname;
                    unsafe { $crate::headers::HeaderName::from_ascii_unchecked( as_str ) }
                }

                fn get_contextual_validator<E>()
                    -> Option<fn(&$crate::headers::HeaderMap<E>) -> $crate::error::Result<()>>
                    where E: $crate::codec::MailEncoder
                {
                    def_headers!{ _PRIV_mk_validator E, $validator }
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
    (_PRIV_mk_validator $E:ident, None) => ({ None });
    (_PRIV_mk_validator $E:ident, $validator:ident) => ({ Some($validator::<$E>) });
    (_PRIV_boolify +) => ({ false });
    (_PRIV_boolify 1) => ({ true });
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
use self::validators::{
    from as validator_from,
    resent_any as validator_resent_any
};
def_headers! {
    test_name: validate_header_names,
    scope: components,
    //RFC 5322:
    1 Date,                    unsafe { "Date"          },  DateTime,       None,
    1 From,                    unsafe { "From"          },  MailboxList,    validator_from,
    1 Sender,                  unsafe { "Sender"        },  Mailbox,        None,
    1 ReplyTo,                 unsafe { "Reply-To"      },  MailboxList,    None,
    1 To,                      unsafe { "To"            },  MailboxList,    None,
    1 Cc,                      unsafe { "Cc"            },  MailboxList,    None,
    1 Bcc,                     unsafe { "Bcc"           },  MailboxList,    None,
    1 MessageId,               unsafe { "Message-Id"    },  MessageID,      None,
    1 InReplyTo,               unsafe { "In-Reply-To"   },  MessageIDList,  None,
    1 References,              unsafe { "References"    },  MessageIDList,  None,
    1 Subject,                 unsafe { "Subject"       },  Unstructured,   None,
    + Comments,                unsafe { "Comments"      },  Unstructured,   None,
    + Keywords,                unsafe { "Keywords"      },  PhraseList,     None,
    + ResentDate,              unsafe { "Resent-Date"   },  DateTime,       validator_resent_any,
    + ResentFrom,              unsafe { "Resent-From"   },  MailboxList,    validator_resent_any,
    + ResentSender,            unsafe { "Resent-Sender" },  Mailbox,        validator_resent_any,
    + ResentTo,                unsafe { "Resent-To"     },  MailboxList,    validator_resent_any,
    + ResentCc,                unsafe { "Resent-Cc"     },  MailboxList,    validator_resent_any,
    + ResentBcc,               unsafe { "Resent-Bcc"    },  OptMailboxList, validator_resent_any,
    + ResentMsgId,             unsafe { "Resent-Msg-Id" },  MessageID,      validator_resent_any,
    + ReturnPath,              unsafe { "Return-Path"   },  Path,           None,
    + Received,                unsafe { "Received"      },  ReceivedToken,  None,
    //RFC 2045:
    1 ContentType,             unsafe { "Content-Type"              }, Mime,             None,
    1 ContentId,               unsafe { "Content-Id"                }, ContentID,        None,
    1 ContentTransferEncoding, unsafe { "Content-Transfer-Encoding" }, TransferEncoding, None,
    1 ContentDescription,      unsafe { "Content-Description"       }, Unstructured,     None,
    //RFC 2183:
    1 ContentDisposition,      unsafe { "Content-Disposition"       }, Disposition, None
}

mod validators {
    use std::collections::HashMap;

    use error::*;
    use codec::{ MailEncoder, MailEncodable};
    use headers::{ HeaderMap, Header, HeaderName };
    use super::{ From, ResentFrom, Sender, ResentSender, ResentDate };

    pub fn from<E>(map: &HeaderMap<E>) -> Result<()>
        where E: MailEncoder
    {
        // Note: we do not care about the quantity of From bodies,
        // nor "other" From bodies
        // (which do not use a MailboxList and we could
        //  therefore not cast to it,
        // whatever header put them in has also put in
        // this bit of validation )
        let needs_sender =
            map.get(From).map(|bodies|
                bodies.filter_map(|res| res.ok()).any(|list| list.len() > 1 )
            ).unwrap_or(false);

        if needs_sender && !map.contains(Sender) {
            bail!("if a multi-mailbox From is used Sender has to be specified");
        }
        Ok(())
    }

    fn validate_resent_block<'a, E>(
            block: &HashMap<HeaderName, &'a MailEncodable<E>>
    ) -> Result<()>
        where E: MailEncoder
    {
        if !block.contains_key(&ResentDate::name()) {
            bail!("each reasond block must have a Resent-Date field");
        }
        let needs_sender =
            //no Resend-From? => no problem
            block.get(&ResentFrom::name())
                //can't cast? => not my problem/responsibility
                .and_then(|tobj| tobj.downcast_ref::<<ResentFrom as Header>::Component>())
                .map(|list| list.len() > 1)
                .unwrap_or(false);

        if needs_sender && !block.contains_key(&ResentSender::name()) {
            bail!("each resent block containing a multi-mailbox Resent-From needs to have a Resent-Sender field too")
        }
        Ok(())
    }

    pub fn resent_any<E>(map: &HeaderMap<E>) -> Result<()>
        where E: MailEncoder
    {
        let resents = map
            .iter()
            .filter(|&(name, _)| name.as_str().starts_with("Resent-"));

        let mut block = HashMap::new();
        for (name, content) in resents {
            if block.contains_key(&name) {
                validate_resent_block(&block)?;
                //create new block
                block = HashMap::new();
            }
            block.insert(name, content);
        }
        validate_resent_block(&block)
    }
}

#[cfg(test)]
mod test {
    use codec::MailEncoderImpl;
    use components::DateTime;
    use headers::{
        HeaderMap,
        From, ResentFrom, ResentTo, ResentDate,
        Sender, ResentSender, Subject
    };

    fn typed(_: &HeaderMap<MailEncoderImpl>){}

    #[test]
    fn from_validation_normal() {
        let mut map = HeaderMap::new();
        map.insert(From, [("Mr. Peté", "pete@nixmail.nixdomain")]).unwrap();
        map.insert(Subject, "Ok").unwrap();
        typed(&map);

        assert_ok!(map.use_contextual_validators());
    }
    #[test]
    fn from_validation_multi_err() {
        let mut map = HeaderMap::new();
        map.insert(From, (
            ("Mr. Peté", "nixperson@nixmail.nixdomain"),
            "a@b.c"
        )).unwrap();
        map.insert(Subject, "Ok").unwrap();
        typed(&map);

        assert_err!(map.use_contextual_validators());
    }

    #[test]
    fn from_validation_multi_ok() {
        let mut map = HeaderMap::new();
        map.insert(From, (
            ("Mr. Peté", "nixperson@nixmail.nixdomain"),
            "a@b.c"
        )).unwrap();
        map.insert(Sender, "abx@d.e").unwrap();
        map.insert(Subject, "Ok").unwrap();
        typed(&map);

        assert_ok!(map.use_contextual_validators());
    }

    #[test]
    fn resent_no_date_err() {
        let mut map = HeaderMap::new();
        map.insert(ResentFrom,["a@b.c"]).unwrap();
        typed(&map);
        assert_err!(map.use_contextual_validators());
    }

    #[test]
    fn resent_with_date() {
        let mut map = HeaderMap::new();
        map.insert(ResentFrom,["a@b.c"]).unwrap();
        map.insert(ResentDate, DateTime::now()).unwrap();
        typed(&map);
        assert_ok!(map.use_contextual_validators());
    }

    #[test]
    fn resent_no_date_err_second_block() {
        let mut map = HeaderMap::new();
        map.insert(ResentDate, DateTime::now()).unwrap();
        map.insert(ResentFrom,["a@b.c"]).unwrap();
        map.insert(ResentTo, ["e@f.d"]).unwrap();
        map.insert(ResentFrom, ["ee@ee.e"]).unwrap();

        typed(&map);
        assert_err!(map.use_contextual_validators());
    }

    #[test]
    fn resent_with_date_second_block() {
        let mut map = HeaderMap::new();
        map.insert(ResentDate, DateTime::now()).unwrap();
        map.insert(ResentFrom,["a@b.c"]).unwrap();
        map.insert(ResentTo, ["e@f.d"]).unwrap();
        map.insert(ResentFrom, ["ee@ee.e"]).unwrap();
        map.insert(ResentDate, DateTime::now()).unwrap();

        typed(&map);
        assert_ok!(map.use_contextual_validators());
    }

    #[test]
    fn resent_multi_mailbox_from_no_sender() {
        let mut map = HeaderMap::new();
        map.insert(ResentDate, DateTime::now()).unwrap();
        map.insert(ResentFrom, ["a@b.c","e@c.d"]).unwrap();
        typed(&map);
        assert_err!(map.use_contextual_validators());
    }

    #[test]
    fn resent_multi_mailbox_from_with_sender() {
        let mut map = HeaderMap::new();
        map.insert(ResentDate, DateTime::now()).unwrap();
        map.insert(ResentFrom, ["a@b.c","e@c.d"]).unwrap();
        map.insert(ResentSender, "a@b.c").unwrap();
        typed(&map);
        assert_ok!(map.use_contextual_validators());
    }
}
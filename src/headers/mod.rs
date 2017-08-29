use std::ops::Deref;
use std::borrow::{ Cow, ToOwned };

use ascii::{ AsciiStr, AsciiChar };

use error::*;
use components::{
    Mailbox, OptMailboxList, MailboxList,
    MessageID, MessageIDList,
    Unstructured, DateTime,
    Path, ReceivedToken,
    TransferEncoding, Disposition,
    Mime, PhraseList, HeaderName

};

use codec::{ MailEncoder, MailEncodable };


include! { concat!( env!( "OUT_DIR" ), "/header_enum.rs.partial" )  }

//FIXME tendentially merge with types::HeaderName to some extend
pub enum HeaderNameRef<'a> {
    Static( &'static AsciiStr ),
    Other( &'a AsciiStr )
}

impl<'a> Deref for HeaderNameRef<'a> {
    type Target = AsciiStr;

    fn deref( &self ) -> &AsciiStr {
        use self::HeaderNameRef::*;
        match self {
            &Static( res ) => res,
            &Other( res ) => res
        }
    }
}

impl<'a> Into<Cow<'static, AsciiStr>> for HeaderNameRef<'a> {
    fn into( self ) -> Cow<'static, AsciiStr> {
        use self::HeaderNameRef::*;
        match self {
            Static( static_ref ) => Cow::Borrowed( static_ref ),
            Other( non_static_ref ) => Cow::Owned( non_static_ref.to_owned() )
        }
    }
}

impl Header {

    pub fn name<'a>( &'a self ) -> HeaderNameRef<'a> {
        use self::Header::*;
        //a match with arms like `Date( .. ) => unsafe { AsciiStr::from_ascii_unchecked( "Date" ) }`
        let fn_impl = include! { concat!( env!( "OUT_DIR" ), "/header_enum_names.rs.partial" )  };
        fn_impl( self )
    }
}

include!( concat!( env!( "OUT_DIR", ), "/mail_encodable_impl.rs.partial" ) );


fn encode_header_helper<T, E>(
    name: &AsciiStr, encodable: &T, encoder: &mut E
) -> Result<()>
    where T: MailEncodable<E>, E: MailEncoder
{
    encoder.write_str( name );
    encoder.write_char( AsciiChar::Colon );
    //any of the following types have a leading [CFWS] so we just "write" it out here
    //NOTE: for some data like text/unstructured the space theoretically belongs to the data
    encoder.write_fws();
    encodable.encode( encoder )
}





//TODO new data interface all components implement some "construction" trait
//  like TryFrom / TryInto
//  1. roll custom Try/FromTraits? if so SEAL them or not?
//  2. how to link header to body
//  3.

//header:
//  Mapping with  HeaderName -> Component, so Stringly  -> Type??
//
// # Case 1
//  .header( "Content-Type", "text/plain; charser=utf8" )
//    1. "Content-Type" -> header_name
//    2. header_name -> Mime
//    3. Mime::my_try_from( "text/plain; charser=utf8" )
//
// # Case 2
//  .header( "X-Custom, "omg this is mowmow" )
//    1. "X-Custom" -> header_name
//    2. header_name -> CustomType
//    3. CustomType::my_try_from( "omg this is mowmow" )
//
//
// In case 1 we might be able to have a static lookup table
// But for case 2 it has to be extensible
//
// The mapping would be  HeaderName -> dyn Encodable+MyTryFrom
// which means any header encoding call would go through a VTable Call.
// Also the construction goes through a VCall too! as we would have to
// have some mapping HeaderName -> MyTryFrom
//
// alternatively we could have a `enum Header { HeaderName(header_value).. }`
// with the last variant being `Other( HeaderName, dyn Encodable+MyTryFrom )`
// we also would need an extensible mapping HeaderName -> MyTryFrom for extension
// BUT we only have to touch it if we hit an extension, therefor this should be
// faster,
//
// Question does a match on string uses a tri as optimization??
//  it should but uh does it?
//
//
// UhOhUhm, TryFrom::from is generic... aka not working with trait objects so this wont work
//   I could limit it to TryFromInput + Into<Input> but this is suboptimal for some components
//   like Email ( +from (display_name, email) +from email ) and list of ... ( +from Vec, Iterator )
//   etc.

// New approach we have type for each header having a associated type
//   but this still does not work well as ther can be multiple from (e.g. Email/ListOfs)

// But we can have a type for Header (names) and accosicate the component type
//   but then we can not have stringly types Headers... duh


mod new_header {
    use std::borrow::Cow;
    use std::any::{ Any, TypeId };
    use std::collections::{ HashMap as Map };
    use std::collections::hash_map::Entry::*;
    use std::marker::PhantomData;
    use std::slice::{ Iter as SliceIter };
    use std::iter::Iterator;

    use error::*;
    use grammar::is_ftext;
    use codec::{ MailEncoder, MailEncodable };
    use ascii::{ AsciiStr, AsciiString };
    pub use ascii::{ AsciiStr as _AsciiStr };

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
    macro_rules! def_headers {
        (
            test_name: $tn:ident,
            scope: $scope:ident,
            $($multi:tt $name:ident, unsafe { $hname:tt }, $component:ident),+
        ) => (
            $(
                pub struct $name;

                impl $crate::headers::new_header::Header for  $name {
                    type Component = $scope::$component;

                    fn name() -> HeaderName {
                        let as_str: &'static str = $hname;
                        unsafe { $crate::headers::new_header::HeaderName::from_ascii_unchecked( as_str ) }
                    }

                    fn can_appear_multiple_times() -> bool {
                        def_headers!(_PRIV_boolify $multi)
                    }
                }
            )+

            #[cfg(test)]
            const HEADER_NAMES: &[ &str ] = &[ $(
                $hname
            ),+ ];

            #[test]
            fn $tn() {
                use $crate::codec::{ MailEncoder, MailEncodable, MailEncoderImpl };
                fn can_be_trait_object<E: MailEncoder, EN: MailEncodable<E>>( v: Option<&EN> ) {
                    let _ = v.map( |en| en as &MailEncodable<E> );
                }
                $(
                    can_be_trait_object::<MailEncoderImpl, $scope::$component>( None );
                )+
                for name in HEADER_NAMES {
                    let mut word_start = true;
                    for char in name.chars() {
                        match char {
                            'a'...'z' => {
                                if word_start {
                                    panic!("invalide header name {}", name);
                                }
                            }
                            'A'...'z' => {
                                if !word_start {
                                    panic!("invalide header name {}", name);
                                }
                                word_start = false;
                            }
                            '0'...'9' => {
                                if word_start {
                                    panic!("invalide header name {}", name);
                                }
                            }
                            '-' => {
                                if word_start {
                                    panic!("invalide header name {}", name);
                                }
                                word_start = true
                            }
                        }
                    }
                }
            }
        );
        (_PRIV_boolify *) => ({ true });
        (_PRIV_boolify 1) => ({ false });
    }

    pub trait Header {
        type Component;

        fn name() -> HeaderName;
        fn can_appear_multiple_times() -> bool {
            false
        }
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
        1 MessageID,               unsafe { "Message-ID"    },  MessageID,
        1 InReplyTo,               unsafe { "In-Reply-To"   },  MessageIDList,
        1 References,              unsafe { "References"    },  MessageIDList,
        1 Subject,                 unsafe { "Subject"       },  Unstructured,
        1 Comments,                unsafe { "Comments"      },  Unstructured,
        1 Keywords,                unsafe { "Keywords"      },  PhraseList,
        * ResentDate,              unsafe { "Resent-Date"   },  DateTime,
        * ResentFrom,              unsafe { "Resent-From"   },  MailboxList,
        * ResentSender,            unsafe { "Resent-Sender" },  Mailbox,
        * ResentTo,                unsafe { "Resent-Sender" },  MailboxList,
        * ResentCc,                unsafe { "Resent-Cc"     },  MailboxList,
        * ResentBcc,               unsafe { "Resent-Bcc"    },  OptMailboxList,
        * ResentMsgID,             unsafe { "Resent-Msg-ID" },  MessageID,
        * ReturnPath,              unsafe { "Return-Path"   },  Path,
        * Received,                unsafe { "Received"      },  ReceivedToken,
        //RFC 2045:
        1 ContentID,               unsafe { "Content-ID"                }, ContentID,
        1 ContentTransferEncoding, unsafe { "Content-Transfer-Encoding" }, TransferEncoding,
        1 ContentDescription,      unsafe { "Content-Description"       }, Unstructured,
        //RFC 2183:
        1 ContentDisposition,      unsafe { "Content-Disposition"       }, Disposition
    }



    //TODO replace with std TryFrom once it is stable
    // (either a hard replace, or a soft replace which implements HeaderTryFrom if TryFrom exist)
    pub trait HeaderTryFrom<T>: Sized {
        fn try_from(val: T) -> Result<Self>;
    }

    pub trait HeaderTryInto<T>: Sized {
        fn try_into(self) -> Result<T>;
    }

    impl<F, T> HeaderTryInto<T> for F where T: HeaderTryFrom<F> {
        fn try_into(self) -> Result<T> {
            T::try_from(self)
        }
    }

    #[derive(Debug, Clone, Eq, PartialEq, Hash)]
    pub struct HeaderName {
        name: &'static AsciiStr
    }

    impl HeaderName {
        pub fn new( name: &'static AsciiStr ) -> Result<Self> {
            HeaderName::validate_name( name )?;
            Ok( HeaderName { name } )
        }
        pub unsafe fn from_ascii_unchecked<B: ?Sized>( name: &'static B ) -> HeaderName
            where B: AsRef<[u8]>
        {
            HeaderName { name: AsciiStr::from_ascii_unchecked( name ) }
        }
    }

    impl HeaderName {

        /// validates if the header name is valid
        fn validate_name( name: &AsciiStr ) -> Result<()> {
            if name.as_str().chars().all( is_ftext ) {
                Ok(())
            } else {
                bail!( "invalide header name" )
            }
        }
    }

    struct HeaderBody<E: MailEncoder> {
        first: Box<MailEncodable<E>>,
        other: Option<Vec<Box<MailEncodable<E>>>>
    }

    pub struct HeaderMap<E: MailEncoder> {
        // the only header which is allowed/meant to appear more than one time is
        // Trace!/Comment?, we _could_ consider using a Name->SingleEncodable mapping and
        // make the multie occurence aspect of Trace part of the trace type,
        // but this could get annoying wrt. to parsing and other custom header
        // which allow this
        //
        // Idea have some kind of wrapper and move this property into the type system
        // we are already abstracting with Trait objects, so why not?
        headers: Map<HeaderName, HeaderBody<E>>

    }

    ///FIXME this does not really need to be static
    impl<E: MailEncoder + 'static> HeaderMap<E> {

        pub fn new() -> Self {
            HeaderMap { headers: Map::new() }
        }

        pub fn get_single<'a ,H>( &'a self ) -> Result<Option<&'a H::Component>>
            where H: Header, H::Component: 'static
        {
            if let Some( body ) = self.headers.get( &H::name() ) {
                downcast_ref::<E, H::Component>( &*body.first )
                    .ok_or_else( ||->Error {
                        "use of different header types with same header name".into() } )
                    .map( |res_ref| Some( res_ref ) )
            } else {
                Ok( None )
            }

        }

        pub fn get<H>( &self ) -> Option<HeaderMultiBodyIter<E, H>>
            where H: Header
        {
            if let Some( body ) = self.headers.get( &H::name() ) {
                Some( HeaderMultiBodyIter::new(
                    &*body.first,
                    body.other.as_ref().map( |o| o.iter() )
                ) )
            } else {
                None
            }
        }


        pub fn insert<H>( &mut self, body: H::Component ) -> Result<()>
            where H: Header, H::Component: MailEncodable<E>
        {
            let tobj: Box<MailEncodable<E>> = Box::new( body );
            let multi = H::can_appear_multiple_times();
            //workaround for not having non lexical live times yet
            {
                if let Some( body ) = self.headers.get_mut( &H::name() ) {
                    let has_multiple = body.other.is_some();
                    if multi != has_multiple {
                        bail!( "multi appearance header combined with single apparence header with same name" );
                    }
                    if multi {
                        //UNWRAP_SAFE: as multi == has_multi == other.is_some() == true
                        body.other.as_mut().unwrap().push( tobj );
                    } else {
                        //override non multi entry
                        body.first = tobj;
                    }
                    return Ok(())
                }
            }

            self.headers.insert( H::name().to_owned(), HeaderBody {
                first: tobj,
                other: if multi {
                    Some( Vec::new() )
                } else {
                    None
                }
            });

            Ok( () )
        }
    }

    pub struct HeaderMultiBodyIter<'a, E: 'a, H> {
        state: InnerHeaderMultiBodyIter<'a, E>,
        _header_type: PhantomData<H>
    }

    impl<'a, E, H> HeaderMultiBodyIter<'a, E, H>
        where E: MailEncoder, H: Header
    {
        fn new(
            first: &'a MailEncodable<E>,
            other: Option<SliceIter<'a, Box<MailEncodable<E>>>>
        ) -> Self {
            HeaderMultiBodyIter {
                state: InnerHeaderMultiBodyIter { first: Some(first), other  },
                _header_type: PhantomData
            }
        }
    }

    struct  InnerHeaderMultiBodyIter<'a, E: 'a> {
        first: Option<&'a MailEncodable<E>>,
        other: Option<SliceIter<'a, Box<MailEncodable<E>>>>
    }

    impl<'a, E, H> Iterator for HeaderMultiBodyIter<'a, E, H>
        where E: MailEncoder, H: Header, H::Component: MailEncodable<E>
    {
        type Item = Result<&'a H::Component>;

        fn next(&mut self) -> Option<Self::Item> {
            use std::any::Any;
            let tobj_item = self.state.first
                .take()
                .or_else( || {
                    self.state.other.as_mut()
                        .and_then( |other| other.next() )
                        .map( |val| &**val )
                });
            tobj_item.map( |tobj| {
                downcast_ref::<E, H::Component>( tobj )
                    .ok_or_else( ||->Error {
                        "use of different header types with same header name".into() } )
            })
        }
    }

    fn downcast_ref<E: MailEncoder, O: Any+'static>( tobj: &MailEncodable<E>) -> Option<&O> {
        if TypeId::of::<O>() == tobj.type_id() {
            Some( unsafe { &*( tobj as *const MailEncodable<E> as *const O) } )
        } else {
            None
        }
    }

//    builder
//        .header::<Subject>( "this is awesome" )
//        .header::<From>( "my@place" )
//
//    builder
//        .header(Subject, "this is awesom" )
//        .header(From, "my@place" )



}



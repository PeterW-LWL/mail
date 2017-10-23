use std::marker::PhantomData;
use std::iter::ExactSizeIterator;
use std::fmt::{self, Debug};
use std::result::{ Result as StdResult };
use std::mem;
use std::collections::HashSet;


use total_order_multi_map::{
    self,
    TotalOrderMultiMap, Meta,
    EntryValues
};

use error::*;

use utils::HeaderTryInto;
use codec::EncodableInHeader;

use super::{
    HeaderName,
    Header,
    SingularHeaderMarker,
    HasHeaderName
};

pub use self::into_iter::*;
mod into_iter;

/// A runtime representations of a `Header` types meta
/// properties like `MAX_COUNT_EQ_1` or `CONTEXTUAL_VALIDATOR`
pub struct HeaderMeta {
    pub max_count_eq_1: bool,
    pub contextual_validator: Option<fn(&HeaderMap) -> Result<()>>
}
//TODO imple PartialEq, Eq and Hash per hand as derive does not work as it adds a wher E: XXX bound

impl Clone for HeaderMeta {

    fn clone(&self) -> Self {
        HeaderMeta {
            max_count_eq_1: self.max_count_eq_1,
            contextual_validator: self.contextual_validator.clone()
        }
    }

}
impl Debug for HeaderMeta {

    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        let usized = self.contextual_validator.clone().map(|x|x as usize);
        fter.debug_struct("HeaderMeta")
            .field("max_count_eq_1", &self.max_count_eq_1)
            .field("contextual_validator", &usized)
            .finish()
    }
}


impl HeaderMeta {

    pub fn from_header_type<H: Header>() -> Self {
        HeaderMeta {
            max_count_eq_1: H::MAX_COUNT_EQ_1,
            contextual_validator: H::CONTEXTUAL_VALIDATOR.clone()
        }
    }

    #[inline]
    pub fn is_compatible(&self, other: &HeaderMeta) -> bool {
        self.check_update(other).is_ok()
    }

    #[inline]
    fn cmp_validator_eq(&self, other: &Self) -> bool {
        let thisone = self.contextual_validator.map(|fnptr| fnptr as usize).unwrap_or(0);
        let thatone = other.contextual_validator.map(|fptr| fptr as usize).unwrap_or(0);
        thisone == thatone
    }
}

impl Meta for HeaderMeta {

    type MergeError = Error;

    fn check_update(&self, other: &Self) -> StdResult<(), Self::MergeError> {
        if self.max_count_eq_1 != other.max_count_eq_1 {
            bail!("trying to mix up Headers with the same name but different quantity limitations");
        }

        if !self.cmp_validator_eq(other) {
            bail!("trying to mix up Headers with same name but different contextual validator");
        }
        Ok(())
    }

    fn update(&mut self, other: Self) {
        //as we don't allow values with different meta no update needed
        mem::drop(other)
    }
}


///
/// # Note
///
/// a number of methods implemented on HeaderMap appear in two variations,
/// one which accepts a type hint (a normally zero sized struct implementing
/// Header) and on which just accepts the type and needs to be called with
/// the turbofish operator. The later one is prefixed by a `_` as the former
/// one is more nice to use, but in some siturations, e.g. when wrapping
/// `HeaderMap` in custom code the only type accepting variations are more
/// usefull.
///
/// ```rust,ignore
/// let _ = map.get(Subject);
/// //is equivalent to
/// let _ = map._get::<Subject>();
/// ```
///
pub struct HeaderMap {
    inner_map: TotalOrderMultiMap<HeaderName, Box<EncodableInHeader>, HeaderMeta>,
}

pub type Iter<'a> = total_order_multi_map::Iter<'a, HeaderName, Box<EncodableInHeader>>;
pub type IterMut<'a> = total_order_multi_map::IterMut<'a, HeaderName, Box<EncodableInHeader>>;
pub type IntoIterWithMeta =
    total_order_multi_map::IntoIterWithMeta<HeaderName, Box<EncodableInHeader>, HeaderMeta>;

impl Debug for HeaderMap {
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "HeaderMap {{ ")?;
        for (key, val_cont) in self.iter() {
            write!(fter, "{}: {:?},", key.as_str(), val_cont)?;
        }
        write!(fter, " }}")
    }
}

impl Default for HeaderMap {
    fn default() -> Self {
        HeaderMap {
            inner_map: Default::default()
        }
    }
}

impl HeaderMap {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn len(&self) -> usize {
        self.inner_map.len()
    }

    /// call each unique contextual validator exactly once with this map as parameter
    ///
    /// If multiple Headers provide the same contextual validator (e.g. the resent headers)
    /// it's still only called once.
    pub fn use_contextual_validators(&self) -> Result<()> {
        let mut already_called = HashSet::new();
        for group in self.inner_map.group_iter() {
            if let Some(validator) = group.meta().contextual_validator {
                if already_called.insert(validator as usize) {
                    (validator)(self)?;
                }
            }
        }
        Ok(())
    }

    /// returns true if the headermap contains a header with the same name
    pub fn contains<H: HasHeaderName>(&self, name: H ) -> bool {
        self.inner_map.contains_key(name.get_name())
    }

    #[inline(always)]
    pub fn get_single<'a ,H>( &'a self, _type_hint: H ) -> Option<Result<&'a H::Component>>
        where H: Header + SingularHeaderMarker,
              H::Component: EncodableInHeader
    {
        self._get_single::<H>()
    }

    ///
    /// Note:
    /// if you implement `SingularHeaderMarker` on a header
    /// which can appear multiple times this function will
    /// just return one of the multiple possible values
    /// (if there are any) with out any guarantees which one
    /// or that multiple call to it will always return the
    /// same one
    pub fn _get_single<'a ,H>( &'a self ) -> Option<Result<&'a H::Component>>
        where H: Header + SingularHeaderMarker,
              H::Component: EncodableInHeader
    {
        self.get_untyped(H::name())
            .map( |mut bodies| {
                //TODO: possible make this a debug only check
                HeaderMeta::from_header_type::<H>()
                    .check_update(bodies.meta())?;
                //UNWRAP_SAFE: we have at last one element
                let untyped = bodies.next().unwrap();
                untyped.downcast_ref::<H::Component>()
                    .ok_or_else( ||->Error {
                        "use of different header types with same header name".into() } )
            } )
    }

    ///
    /// Accepts both `HeaderName` or a type implementing `Header`.
    ///
    #[inline]
    pub fn get_untyped<H: HasHeaderName>( &self, name: H ) -> Option<UntypedBodies> {
        self.inner_map.get( name.get_name() )
    }

    #[inline(always)]
    pub fn get<H>( &self, _type_hint: H) -> Option<TypedBodies<H>>
        where H: Header, H::Component: EncodableInHeader
    {
        self._get::<H>()
    }

    pub fn _get<H>( &self ) -> Option<TypedBodies<H>>
        where H: Header, H::Component: EncodableInHeader
    {
        self.get_untyped( H::name() )
            .map( |untyped| untyped.into() )
    }

    /// Inserts given header into the header map.
    ///
    /// Returns the count of headers with the given name after inserting
    /// this header
    ///
    /// Note:
    /// the original signature did not take the header as a
    /// parameter but just as a type, now it takes it
    /// as a parameter as `.insert(ContentType, "text/plain")` is
    /// much more userfrindly then `.insert::<ContentType, _>( "text/plain" )`.
    /// the original signature is still available as `_insert` as it
    /// is usefull for some circumstances where it is bothersome to
    /// create a (normally zero-sized) Header instance as type hint
    #[inline(always)]
    pub fn insert<H, C>( &mut self, _htype_hint: H, hbody: C ) -> Result<usize>
        where H: Header,
              H::Component: EncodableInHeader,
              C: HeaderTryInto<H::Component>
    {
        self._insert::<H, C>( hbody )
    }

    /// works like `HeaderMap::insert`, except that no header instance as
    /// type hint has to (nor can) be passed in
    ///
    /// Returns the count of headers with the given name after inserting
    /// this header.
    #[inline]
    pub fn _insert<H, C>( &mut self,  hbody: C ) -> Result<usize>
        where H: Header,
              H::Component: EncodableInHeader,
              C: HeaderTryInto<H::Component>
    {
        let hbody: H::Component = hbody.try_into()?;
        let tobj: Box<EncodableInHeader> = Box::new( hbody );
        let name = H::name();
        let meta = HeaderMeta::from_header_type::<H>();
        self.inner_map.insert(name, tobj, meta)
            .map_err(|(hn,_,_,err)| {
                err.chain_err(||ErrorKind::FailedToAddHeader(hn.as_str()))
            })
    }

    /// # Error
    ///
    /// Returns a MultopleErrors error containing all errors for
    /// each header which in `other` which could not be added to
    /// `self`. Note that if an error does occure it is assured that
    /// any header added to `self` during this call to `extend` is
    /// removed again before `extend` returns.
    ///
    pub fn extend( &mut self, other: HeaderMap )
        -> Result<&mut Self>
    {
        let prev_len = self.len();
        let res = self.inner_map.extend(other.into_iter_with_meta());
        match res {
            Ok(()) => Ok(self),
            Err(errs) => {
                //combine errors
                let errs = errs.into_iter().map(|(hn, _comp, _meta, error)| {
                    error.chain_err(||ErrorKind::FailedToAddHeader(hn.as_str()))
                }).collect::<Vec<_>>();
                let error = ErrorKind::MultipleErrors(errs.into());

                while self.len() > prev_len {
                    let _ = self.inner_map.pop();
                }
                Err(error.into())
            }
        }
    }

    /// remove all headers with the given header name
    ///
    /// returns true, if at last one element was removed
    ///
    /// # Example
    ///
    #[inline]
    pub fn remove_by_name<H: HasHeaderName>(&mut self, name: H ) -> bool {
        self.inner_map.remove_all(name.get_name())
    }


    #[inline]
    pub fn iter(&self) -> Iter {
        self.inner_map.iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> IterMut {
        self.inner_map.iter_mut()
    }

    #[inline]
    pub fn into_iter_with_meta(self) -> IntoIterWithMeta {
        self.inner_map.into_iter_with_meta()
    }
}


pub type UntypedBodies<'a> = EntryValues<'a, EncodableInHeader, HeaderMeta>;


pub struct TypedBodies<'a, H>
    where H: Header,
          H::Component: EncodableInHeader
{
    inner: UntypedBodies<'a>,
    _marker: PhantomData<H>
}

impl<'a, H> From<UntypedBodies<'a>> for TypedBodies<'a, H>
    where H: Header,
          H::Component: EncodableInHeader
{
    fn from(untyped: UntypedBodies<'a>) -> Self {
        TypedBodies { inner: untyped, _marker: PhantomData }
    }
}

impl<'a, H> TypedBodies<'a, H>
    where H: Header,
          H::Component: EncodableInHeader
{
    pub fn new(inner: UntypedBodies<'a>) -> Self {
        TypedBodies {
            inner,
            _marker: PhantomData
        }
    }
    pub fn meta(&self) -> &HeaderMeta {
        self.inner.meta()
    }
}

impl<'a, H> Iterator for TypedBodies<'a, H>
    where H: Header,
          H::Component: EncodableInHeader
{
    type Item = Result<&'a H::Component>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
            .map( |tobj| {
                tobj.downcast_ref::<H::Component>()
                    .ok_or_else( || -> Error {
                        "use of different header types with same header name".into() } )
            } )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, H> ExactSizeIterator for TypedBodies<'a, H>
    where H: Header,
          H::Component: EncodableInHeader
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, H> Clone for TypedBodies<'a, H>
    where H: Header,
          H::Component: EncodableInHeader
{
    fn clone(&self) -> Self {
        TypedBodies::new(self.inner.clone())
    }
}

impl<'a, H> Debug for TypedBodies<'a, H>
    where H: Header,
          H::Component: EncodableInHeader
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        fter.debug_struct("TypedBodies")
            .field("inner", &self.inner)
            .finish()
    }
}


#[macro_export]
macro_rules! headers {
    ($($header:ty : $val:expr),*) => ({
        //FIXME use catch block once aviable
        (|| -> $crate::error::Result<HeaderMap> {
            let mut map = $crate::headers::HeaderMap::new();
            $(
                map._insert::<$header, _>( $val )?;
            )*
            Ok( map )
        })()
    });
}




#[cfg(test)]
mod test {
    use soft_ascii_string::SoftAsciiStr;

    use super::*;
    use components::RawUnstructured;
    use self::good_headers::*;
    use self::bad_headers::{
        Subject as BadSubject,
        Comments as BadComments
    };

    use utils::HeaderTryFrom;
    use codec::{EncodableInHeader, EncodeHandle};

    #[derive(Debug, Clone, Eq, PartialEq, Hash)]
    pub struct OtherComponent;

    impl HeaderTryFrom<()> for OtherComponent {
        fn try_from(_: ()) -> Result<OtherComponent> {
            Ok(OtherComponent)
        }
    }
    impl EncodableInHeader for OtherComponent {
        fn encode(&self, _encoder:  &mut EncodeHandle) -> Result<()> {
            bail!("encoding is not implemented")
        }
    }


    mod good_headers {
        use components;
        def_headers! {
            test_name: validate_header_names,
            scope: components,
            1 Subject, unchecked { "Subject" }, RawUnstructured, None,
            + Comments, unchecked { "Comments" }, RawUnstructured, None
        }
    }

    mod bad_headers {
        def_headers! {
            test_name: validate_header_names,
            scope: super,
            1 Subject,  unchecked { "Subject" },  OtherComponent, None,
            + Comments, unchecked { "Comments" }, OtherComponent, None
        }
    }

    const TEXT_1: &str = "Random stuff XD";
    const TEXT_2: &str = "Having a log of fun, yes a log!";

    #[test]
    fn headers_macro() {
        let headers = headers! {
            Comments: TEXT_1,
            Subject: TEXT_2
        }.unwrap();


        let count = headers
            // all headers _could_ have multiple values, through neither
            // ContentType nor Subject do have multiple value
            .get(Comments)
            .expect( "where did the header go?" )
            .map( |h: Result<&RawUnstructured>| {
                let v = h.expect( "the trait object to be downcastable to RawUnstructured" );
                assert_eq!(v.as_str(), TEXT_1);
            })
            .count();
        assert_eq!( 1, count );

        let count = headers
            .get(Subject)
            .expect( "content type header must be present" )
            .map( |h: Result<&RawUnstructured>| {
                let val = h.expect( "the trait object to be downcastable to H::Component" );
                assert_eq!(val.as_str(), TEXT_2);
            })
            .count();
        assert_eq!( 1, count );
    }

    #[test]
    fn get_single() {
        let headers = headers! {
            Subject: "abc"
        }.unwrap();

        assert_eq!(
            "abc",
            headers.get_single(Subject)
                .unwrap()//Some
                .unwrap()//Result
                .as_str()
        );
    }

    #[test]
    fn get_single_cast_error() {
        let headers = headers! {
            Subject: "abc"
        }.unwrap();

        let res = headers.get_single(BadSubject);
        assert_err!( res.expect("where did the header go?") );
    }

    #[test]
    fn get() {
        let headers = headers! {
            Subject: "abc",
            Comments: "1st",
            BadComments: ()
        }.unwrap();


        let mut res = headers.get(Comments)
            .unwrap();

        assert_eq!(res.size_hint(), (2, Some(2)));

        assert_eq!(
            "1st",
            assert_ok!(res.next().unwrap()).as_str()
        );

        assert_err!(res.next().unwrap());

        assert!( res.next().is_none() )

    }

    #[test]
    fn get_untyped() {
        let headers = headers! {
            Subject: "abc",
            Comments: "1st",
            BadComments: ()
        }.unwrap();


        let res = headers.get_untyped(Subject::name())
            .unwrap()
            .map(|entry| entry.downcast_ref::<RawUnstructured>().unwrap().as_str() )
            .collect::<Vec<_>>();

        assert_eq!(
            res.as_slice(),
            &[ "abc" ]
        );

        let mut res = headers.get_untyped(Comments::name()).unwrap();

        assert_eq!((2, Some(2)), res.size_hint());

        assert_eq!(
            res.next().unwrap().downcast_ref::<RawUnstructured>().unwrap().as_str(),
            "1st"
        );

        assert_eq!((1, Some(1)), res.size_hint());

        assert_eq!(
            res.next().unwrap().downcast_ref::<OtherComponent>().unwrap(),
            &OtherComponent
        );

        assert!(res.next().is_none());
    }

    #[test]
    fn fmt_debug() {
        let headers = headers! {
            Subject: "hy there"
        }.unwrap();

        let res = format!("{:?}", headers);
        assert_eq!(
            "HeaderMap { Subject: RawUnstructured { text: Input(Owned(\"hy there\")) }, }",
            res.as_str()
        );
    }

    #[test]
    fn extend_keeps_order() {
        let mut headers = headers! {
            XComment: "ab@c"
        }.unwrap();

        headers.extend( headers! {
            Subject: "hy there",
            Comments: "magic+spell"
        }.unwrap() ).unwrap();

        assert_eq!(
            &[
                "X-Comment",
                "Subject",
                "Comments"
            ],
            headers.into_iter()
                .map(|(name, _val)| name.as_str())
                .collect::<Vec<_>>()
                .as_slice()
        );
    }


    #[test]
    fn remove_1() {
        let mut headers = headers!{
            Comments: "a",
            Subject: "b",
            Comments: "c",
            Comments: "d"
        }.unwrap();

        assert_eq!( false, headers.remove_by_name( XComment::name() ) );
        assert_eq!( true, headers.remove_by_name( Subject::name() ) );

        assert_eq!( 3, headers.iter().count() );

        let values = headers.get(Comments)
            .unwrap()
            .map(|comp| comp.unwrap().as_str() )
            .collect::<Vec<_>>();

        assert_eq!(
            &[ "a", "c", "d" ],
            values.as_slice()
        )
    }

    #[test]
    fn remove_2() {
        let mut headers = headers!{
            Comments: "a",
            Subject: "b",
            Comments: "c",
            Comments: "d"
        }.unwrap();

        assert_eq!( true, headers.remove_by_name( Comments::name() ) );
        assert_eq!( false, headers.remove_by_name( Comments::name() ) );

        assert_eq!( 1, headers.iter().count() );

        let values = headers.get(Subject)
            .unwrap()
            .map(|comp| comp.unwrap().as_str() )
            .collect::<Vec<_>>();

        assert_eq!(
            &[ "b" ],
            values.as_slice()
        );
    }

    struct XComment;
    impl Header for XComment {
        const MAX_COUNT_EQ_1: bool = false;
        type Component = RawUnstructured;

        fn name() -> HeaderName {
            HeaderName::new(SoftAsciiStr::from_str_unchecked("X-Comment")).unwrap()
        }

        const CONTEXTUAL_VALIDATOR: Option<fn(&HeaderMap)-> Result<()>> =
            Some(__validator);
    }

    //some stupid but simple validator
    fn __validator(map: &HeaderMap) -> Result<()> {
        if map.get_untyped(Comments::name()).is_some() {
            bail!("can't have X-Comment and Comments in same mail")
        }
        Ok(())
    }

    #[test]
    fn contains_works() {
        let map = headers! {
            Subject: "soso"
        }.unwrap();

        assert_eq!( true, map.contains(Subject::name()) );
        assert_eq!( true, map.contains(Subject) );
        assert_eq!( false, map.contains(Comments::name()) );
        assert_eq!( false, map.contains(Comments) );
    }

    #[test]
    fn use_validator_ok() {
        let map = headers! {
            XComment: "yay",
            Subject: "soso"
        }.unwrap();

        assert_ok!(map.use_contextual_validators());
    }

    #[test]
    fn use_validator_err() {
        let map = headers! {
            XComment: "yay",
            Comments: "oh no",
            Subject: "soso"
        }.unwrap();

        assert_err!(map.use_contextual_validators());
    }

    #[test]
    fn has_len() {
        let map = headers! {
            XComment: "yay",
            Comments: "oh no",
            Subject: "soso"
        }.unwrap();

        assert_eq!(3, map.len());
    }
}
use std::marker::PhantomData;
use std::iter::ExactSizeIterator;
use std::fmt::{self, Debug};
use std::result::{ Result as StdResult };
use std::mem;
use std::collections::HashSet;


use external::idotom::{
    self,
    Idotom, Meta,
    EntryValues
};

use error::*;

use utils::HeaderTryInto;
use codec::{
    MailEncoder,
    MailEncodable
};

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
pub struct HeaderMeta<E: MailEncoder> {
    pub max_count_eq_1: bool,
    pub contextual_validator: Option<fn(&HeaderMap<E>) -> Result<()>>
}
//TODO imple PartialEq, Eq and Hash per hand as derive does not work as it adds a wher E: XXX bound

impl<E> Clone for HeaderMeta<E>
    where E: MailEncoder
{
    fn clone(&self) -> Self {
        HeaderMeta {
            max_count_eq_1: self.max_count_eq_1,
            contextual_validator: self.contextual_validator.clone()
        }
    }

}
impl<E> Debug for HeaderMeta<E>
    where E: MailEncoder
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        let usized = self.contextual_validator.clone().map(|x|x as usize);
        fter.debug_struct("HeaderMeta")
            .field("max_count_eq_1", &self.max_count_eq_1)
            .field("contextual_validator", &usized)
            .finish()
    }
}


impl<E> HeaderMeta<E>
    where E: MailEncoder
{
    pub fn from_header_type<H: Header>() -> Self {
        HeaderMeta {
            max_count_eq_1: H::MAX_COUNT_EQ_1,
            contextual_validator: H::get_contextual_validator()
        }
    }

    #[inline]
    pub fn is_compatible(&self, other: &HeaderMeta<E>) -> bool {
        self.check_update(other).is_ok()
    }

    #[inline]
    fn cmp_validator_eq(&self, other: &Self) -> bool {
        let thisone = self.contextual_validator.map(|fnptr| fnptr as usize).unwrap_or(0);
        let thatone = other.contextual_validator.map(|fptr| fptr as usize).unwrap_or(0);
        thisone == thatone
    }
}

impl<E> Meta for HeaderMeta<E>
    where E: MailEncoder
{
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
pub struct HeaderMap<E: MailEncoder> {
    inner_map: Idotom<HeaderName, Box<MailEncodable<E>>, HeaderMeta<E>>,
}

pub type Iter<'a, E> = idotom::Iter<'a, HeaderName, Box<MailEncodable<E>>>;
pub type IterMut<'a, E> = idotom::IterMut<'a, HeaderName, Box<MailEncodable<E>>>;
pub type IntoIterWithMeta<E> =
    idotom::IntoIterWithMeta<HeaderName, Box<MailEncodable<E>>, HeaderMeta<E>>;

impl<E> Debug for HeaderMap<E>
    where E: MailEncoder
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "HeaderMap {{ ")?;
        for (key, val_cont) in self.iter() {
            write!(fter, "{}: {:?},", key.as_str(), val_cont)?;
        }
        write!(fter, " }}")
    }
}

impl<E> Default for HeaderMap<E>
    where E: MailEncoder
{
    fn default() -> Self {
        HeaderMap {
            inner_map: Default::default()
        }
    }
}

impl<E> HeaderMap<E>
    where E: MailEncoder
{
    pub fn new() -> Self {
        Default::default()
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
              H::Component: MailEncodable<E>
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
              H::Component: MailEncodable<E>
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
    pub fn get_untyped<H: HasHeaderName>( &self, name: H ) -> Option<UntypedBodies<E>> {
        self.inner_map.get( name.get_name() )
    }

    #[inline(always)]
    pub fn get<H>( &self, _type_hint: H) -> Option<TypedBodies<E, H>>
        where H: Header, H::Component: MailEncodable<E>
    {
        self._get::<H>()
    }

    pub fn _get<H>( &self ) -> Option<TypedBodies<E, H>>
        where H: Header, H::Component: MailEncodable<E>
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
              H::Component: MailEncodable<E>,
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
              H::Component: MailEncodable<E>,
              C: HeaderTryInto<H::Component>
    {
        let hbody: H::Component = hbody.try_into()?;
        let tobj: Box<MailEncodable<E>> = Box::new( hbody );
        let name = H::name();
        let meta = HeaderMeta::from_header_type::<H>();
        self.inner_map.insert(name, tobj, meta)
            .map_err(|(hn,_,_,err)| {
                err.chain_err(||ErrorKind::FailedToAddHeader(hn.as_str()))
            })
    }

    //TODO error description from Idotom::extend
    /// # Error
    ///
    pub fn extend( &mut self, other: HeaderMap<E> )
        -> StdResult<&mut Self, Vec<(HeaderName, Box<MailEncodable<E>>, HeaderMeta<E>, Error)>>
    {
        self.inner_map
            .extend(other.into_iter_with_meta())
            .map(|()| self)
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
    pub fn iter(&self) -> Iter<E> {
        self.inner_map.iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<E> {
        self.inner_map.iter_mut()
    }

    #[inline]
    pub fn into_iter_with_meta(self) -> IntoIterWithMeta<E> {
        self.inner_map.into_iter_with_meta()
    }
}


pub type UntypedBodies<'a, E> = EntryValues<'a, MailEncodable<E>, HeaderMeta<E>>;


pub struct TypedBodies<'a, E, H>
    where E: MailEncoder,
          H: Header,
          H::Component: MailEncodable<E>
{
    inner: UntypedBodies<'a, E>,
    _marker: PhantomData<H>
}

impl<'a, E, H> From<UntypedBodies<'a, E>> for TypedBodies<'a, E, H>
    where E: MailEncoder,
          H: Header,
          H::Component: MailEncodable<E>
{
    fn from(untyped: UntypedBodies<'a, E>) -> Self {
        TypedBodies { inner: untyped, _marker: PhantomData }
    }
}

impl<'a, E, H> TypedBodies<'a, E, H>
    where E: MailEncoder,
          H: Header,
          H::Component: MailEncodable<E>
{
    pub fn new(inner: UntypedBodies<'a, E>) -> Self {
        TypedBodies {
            inner,
            _marker: PhantomData
        }
    }
    pub fn meta(&self) -> &HeaderMeta<E> {
        self.inner.meta()
    }
}

impl<'a, E, H> Iterator for TypedBodies<'a, E, H>
    where E: MailEncoder,
          H: Header,
          H::Component: MailEncodable<E>
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

impl<'a, E, H> ExactSizeIterator for TypedBodies<'a, E, H>
    where E: MailEncoder,
          H: Header,
          H::Component: MailEncodable<E>
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a, E, H> Clone for TypedBodies<'a, E, H>
    where E: MailEncoder,
          H: Header,
          H::Component: MailEncodable<E>
{
    fn clone(&self) -> Self {
        TypedBodies::new(self.inner.clone())
    }
}

impl<'a, E, H> Debug for TypedBodies<'a, E, H>
    where E: MailEncoder,
          H: Header,
          H::Component: MailEncodable<E>
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
        (|| -> $crate::error::Result<HeaderMap<_>> {
            let mut map = $crate::headers::HeaderMap::<_>::new();
            $(
                map._insert::<$header, _>( $val )?;
            )*
            Ok( map )
        })()
    });
}




#[cfg(test)]
mod test {
    use codec::MailEncoderImpl;
    use components::{
        Mime, Unstructured,
        MailboxList
    };
    use headers::{
        ContentType, Subject,
        From, To,
        Comments
    };

    use super::*;

    use self::bad_headers::{
        Subject as BadSubject,
        Comments as BadComments
    };

    mod bad_headers {
        use components;
        def_headers! {
            test_name: validate_header_names,
            scope: components,
            1 Subject, unsafe { "Subject" }, Mime, None,
            + Comments, unsafe { "Comments" }, Mime, None
        }
    }


    fn typed(_v: &HeaderMap<MailEncoderImpl>) {}

    #[test]
    fn headers_macro() {
        let headers = headers! {
            ContentType: "text/plain; charset=us-ascii",
            Subject: "Having a lot of fun",
            From: [
                ("Bla Blup", "bla.blub@not.a.domain")
            ]
        }.unwrap();


        let count = headers
            // all headers _could_ have multiple values, through neither
            // ContentType nor Subject do have multiple value
            .get(ContentType)
            .expect( "content type header must be present" )
            .map( |h: Result<&Mime>| {
                // each of the multiple values could have a different
                // type then H::Component
                h.expect( "the trait object to be downcastable to H::Component" );
            })
            .count();
        assert_eq!( 1, count );

        let count = headers
            .get(Subject)
            .expect( "content type header must be present" )
            .map( |h: Result<&Unstructured>| {
                h.expect( "the trait object to be downcastable to H::Component" );
            })
            .count();
        assert_eq!( 1, count );

        let count = headers
            .get(From)
            .expect( "content type header must be present" )
            .map( |h: Result<&MailboxList>| {
                h.expect( "the trait object to be downcastable to H::Component" );
            })
            .count();
        assert_eq!( 1, count );

        typed(&headers);
    }

    #[test]
    fn get_single() {
        let headers = headers! {
            Subject: "abc"
        }.unwrap();

        typed(&headers);

        assert_eq!( false, headers.get_single(From).is_some() );
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

        typed(&headers);

        let res = headers.get_single(BadSubject);
        assert_eq!( true, res.is_some() );
        assert_err!( res.unwrap() );
    }

    #[test]
    fn get() {
        let headers = headers! {
            Subject: "abc",
            Comments: "1st",
            BadComments: "text/plain"
        }.unwrap();

        typed(&headers);

        let mut res = headers.get(Comments)
            .unwrap();

        assert_eq!((2, Some(2)), res.size_hint());

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
            BadComments: "text/plain"
        }.unwrap();

        typed(&headers);

        let res = headers.get_untyped(Subject::name())
            .unwrap()
            .map(|entry| entry.downcast_ref::<Unstructured>().unwrap().as_str() )
            .collect::<Vec<_>>();

        assert_eq!(
            &[ "abc" ],
            res.as_slice()
        );

        let mut res = headers.get_untyped(Comments::name()).unwrap();

        assert_eq!((2, Some(2)), res.size_hint());

        assert_eq!(
            "1st",
            res.next().unwrap().downcast_ref::<Unstructured>().unwrap().as_str()
        );

        assert_eq!((1, Some(1)), res.size_hint());

        assert_eq!(
            "text/plain".to_owned(),
            format!("{}", res.next().unwrap().downcast_ref::<Mime>().unwrap())
        );

        assert!(res.next().is_none());
    }

    #[test]
    fn fmt_debug() {
        let headers = headers! {
            Subject: "hy there"
        }.unwrap();
        typed(&headers);

        let res = format!("{:?}", headers);
        assert_eq!(
            "HeaderMap { Subject: Unstructured { text: Input(Owned(\"hy there\")) }, }",
            res.as_str()
        );
    }

    #[test]
    fn extend_keeps_order() {
        let mut headers = headers! {
            To: [ "ab@c" ]
        }.unwrap();

        headers.extend( headers! {
            Subject: "hy there",
            From: [ "magic@spell" ]
        }.unwrap() ).unwrap();

        typed(&headers);

        assert_eq!(
            &[
                "To",
                "Subject",
                "From"
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

        typed(&headers);

        assert_eq!( false, headers.remove_by_name( From::name() ) );
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

        typed(&headers);

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
        type Component = Unstructured;

        fn name() -> HeaderName {
            HeaderName::new(ascii_str!(X Minus C o m m e n t )).unwrap()
        }

        fn get_contextual_validator<E>() -> Option<fn(&HeaderMap<E>) -> Result<()>>
            where E: MailEncoder
        {
            //some stupid but simple validator
            fn validator<E: MailEncoder>(map: &HeaderMap<E>) -> Result<()> {
                if map.get_untyped(Comments::name()).is_some() {
                    bail!("can't have X-Comment and Comments in same mail")
                }
                Ok(())
            }
            Some(validator::<E>)
        }
    }

    #[test]
    fn contains_works() {
        let map = headers! {
            Subject: "soso"
        }.unwrap();
        typed(&map);

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
        typed(&map);

        assert_ok!(map.use_contextual_validators());
    }

    #[test]
    fn use_validator_err() {
        let map = headers! {
            XComment: "yay",
            Comments: "oh no",
            Subject: "soso"
        }.unwrap();
        typed(&map);

        assert_err!(map.use_contextual_validators());
    }
}
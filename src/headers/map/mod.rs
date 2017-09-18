use std::fmt;
use std::marker::PhantomData;
use std::collections::{ HashMap as Map, HashMap, hash_map };
use std::iter::{ self as std_iter, Iterator};
use std::slice::{ Iter as SliceIter };

//reexport for headers macro
pub use ascii::{ AsciiStr as _AsciiStr };

use error::*;
use utils::HeaderTryInto;
use codec::{ MailEncoder, MailEncodable };

use super::{
    HeaderName,
    Header,
    SingularHeaderMarker,
    HasHeaderName
};

mod into_iter;
pub use self::into_iter::*;
mod iter;
pub use self::iter::*;

pub struct HeaderMap<E: MailEncoder> {
    // the only header which is allowed/meant to appear more than one time is
    // Trace!/Comment?, we _could_ consider using a Name->SingleEncodable mapping and
    // make the multie occurence aspect of Trace part of the trace type,
    // but this could get annoying wrt. to parsing and other custom header
    // which allow this
    //
    // Idea have some kind of wrapper and move this property into the type system
    // we are already abstracting with Trait objects, so why not?
    //headers: Map<HeaderName, HeaderBodies<E>>,
    // we want to keep and reproduce the insertion order of
    // all headers
    header_vec: Vec<(HeaderName, Box<MailEncodable<E>>)>,
    // we don't want to search for a header when accessing it,
    // we also have to kow if such a header is set etc.
    header_map: Map<HeaderName, HeaderBodies<E>>,

    //OTPIMIZE: I think there are optional hashers better suited for a set of functions
    //WORKAROUND: for rust limitations? bug? we cast the function pointer
    // to usize and use it as key
    contextual_validators: HashMap<usize, fn(&HeaderMap<E>) -> Result<()>>
}



struct HeaderBodies<E: MailEncoder> {
    first: *mut MailEncodable<E>,
    other: Option<Vec<*mut MailEncodable<E>>>
}

type ValidatorIter<'a, E> = std_iter::Map<
    hash_map::Iter<'a,usize, fn(&HeaderMap<E>)->Result<()>>,
    fn((&'a usize, &'a fn(&HeaderMap<E>)->Result<()>)) -> fn(&HeaderMap<E>)->Result<()>
>;


impl<E: MailEncoder> HeaderMap<E> {

    pub fn new() -> Self {
        HeaderMap {
            header_vec: Vec::new(),
            header_map: Map::new(),
            contextual_validators: HashMap::new()
        }
    }

    /// remove all contextual validators
    pub fn clear_contextual_validators(&mut self) {
        self.contextual_validators.clear()
    }

    /// add an additional contextual validator
    pub fn add_contextual_validator(&mut self, vfn: fn(&Self) -> Result<()>) {
        self.contextual_validators.insert(vfn as usize, vfn);
    }

    pub fn iter_contextual_validators(&self) -> ValidatorIter<E> {
        fn map_fn<'a, E: MailEncoder>(
            key_val: (
                &'a usize,
                &'a for<'b> fn(&'b HeaderMap<E>)-> Result<()>
            )
        ) -> for<'b> fn(&'b HeaderMap<E>)->Result<()> {
            *key_val.1
        }
        self.contextual_validators.iter().map(map_fn)
    }

    pub fn use_contextual_validators(&self) -> Result<()> {
        for validator in self.iter_contextual_validators() {
            (validator)(self)?;
        }
        Ok(())
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
        self.get_bodies( H::name() )
            .map( |body| {
                //SAFE: all pointers are always valid and borrowing rules are
                //  indirectly enforced by the `&self` borrow
                let as_ref = unsafe { &*body.first };
                as_ref.downcast_ref::<H::Component>()
                    .ok_or_else( ||->Error {
                        "use of different header types with same header name".into() } )
            })
    }


    #[inline(always)]
    pub fn get<H>( &self, _type_hint: H) -> Option<TypedMultiBodyIter<E, H>>
        where H: Header, H::Component: MailEncodable<E>
    {
        self._get::<H>()
    }

    pub fn _get<H>( &self ) -> Option<TypedMultiBodyIter<E, H>>
        where H: Header, H::Component: MailEncodable<E>
    {
        self.get_untyped( H::name() )
            .map( |untyped| untyped.with_typing() )
    }

    pub fn get_untyped<H: HasHeaderName>( &self, name: H ) -> Option<UntypedMultiBodyIter<E>> {
        if let Some( body ) = self.get_bodies( name.get_name() ) {
            Some( UntypedMultiBodyIter::new(
                //SAFE: all pointers always point to valid data, and the
                // borrow aspects (no mut borrow) are archived through the
                // &self borrow
                unsafe { &*body.first },
                body.other.as_ref().map( |o| o.iter() )
            ) )
        } else {
            None
        }
    }

    /// As this method requires a `&self` borrow it
    /// assures that there won't be any `&mut` borrows,
    /// neither based on `header_vec` nor `header_map`
    /// through there _could_ be other non-mut borrows
    fn get_bodies( &self, name: HeaderName ) -> Option<&HeaderBodies<E>> {
        self.header_map.get( &name )
    }

    /// returns true if the headermap contains a header with the same name
    pub fn contains<H: HasHeaderName>(&self, name: H ) -> bool {
        self.header_map.contains_key(&name.get_name())
    }


    /// # Note
    /// If `extend` fails some values of `other` might already have
    /// been added to this map but non of the contextual validator have
    /// been added yet.
    pub fn extend( &mut self, other: HeaderMap<E> ) -> Result<&mut Self> {
        let HeaderMap { header_vec, header_map, contextual_validators } = other;

        let multi_state = header_map.iter()
            .map( |(name, bodies)| (name, bodies.other.is_some()) )
            .collect::<Map<_,_>>();

        for (name, body) in header_vec.into_iter() {
            //UNWRAP_SAFE: any header in header_vec also appears in header_map
            let is_multi = *multi_state.get(&name).unwrap();
            self.insert_trait_object( name, body, is_multi)?;
        }

        self.contextual_validators.extend(contextual_validators);
        Ok( self )
    }

    /// Inserts given header into the header map,
    /// including the contextual validator which
    /// might be returned by the `Header::get_contextual_validator`
    /// function
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
    pub fn insert<H, C>( &mut self, _htype_hint: H, hbody: C ) -> Result<()>
        where H: Header,
              H::Component: MailEncodable<E>,
              C: HeaderTryInto<H::Component>
    {
        self._insert::<H, C>( hbody )
    }

    /// works like `HeaderMap::insert` except that no header instance as
    /// type hint has to (nor can) be passed in
    #[inline]
    pub fn _insert<H, C>( &mut self,  hbody: C ) -> Result<()>
        where H: Header,
              H::Component: MailEncodable<E>,
              C: HeaderTryInto<H::Component>
    {
        let hbody: H::Component = hbody.try_into()?;
        if let Some(validator) = H::get_contextual_validator() {
            self.add_contextual_validator(validator);
        }
        let tobj: Box<MailEncodable<E>> = Box::new( hbody );
        self.insert_trait_object( H::name(), tobj, H::CAN_APPEAR_MULTIPLE_TIMES )
    }

    fn insert_trait_object(
        &mut self,
        name: HeaderName,
        mut tobj: Box<MailEncodable<E>>,
        can_appear_multiple_times: bool
    ) -> Result<()> {
        //SAFTY: we get a second pointer, while using two at a time is unsafe
        //  the mere existence is not a problem

        let obj_ptr = (&mut *tobj) as *mut MailEncodable<E>;
        self._insert_trait_object_to_map(name, obj_ptr, can_appear_multiple_times)?;
        //only if we succesfull inserted it to the map can we insert it to the vec
        self.header_vec.push( (name, tobj) );
        Ok( () )
    }

    fn _insert_trait_object_to_map(
        &mut self,
        name: HeaderName,
        obj_ptr: *mut MailEncodable<E>,
        can_appear_multiple_times: bool
    ) -> Result<()> {
        {
            if let Some( body ) = self.header_map.get_mut( &name ) {
                if !can_appear_multiple_times {
                    bail!( "field already set and field can appear at most one time" );
                }
                if let Some( other ) = body.other.as_mut() {
                    other.push( obj_ptr );
                    return Ok(())
                } else {
                    bail!( concat!( "multi appearance header combined with single ",
                        "apparence header with same name" ) );
                }
            }
        }

        let empty_other = if can_appear_multiple_times {
            Some( Vec::new() )
        } else {
            None
        };

        self.header_map.insert( name, HeaderBodies {
            first: obj_ptr,
            other: empty_other
        } );

        Ok( () )
    }

    //FIXME use SmallVac/StackVac or whaterver provides the smal vec optimization
    //TODO once drain_filter is stable (rust #43244) use it and return a Vector
    // of removed trait objects
    /// remove all headers with the given header name
    ///
    /// returns true, if at last one element was removed
    ///
    /// # Example
    ///
    /// # Note
    ///
    /// because of the way the insertion order
    /// is stored/remembered removing element is,
    /// in comparsion to e.g. an hash map, not
    /// a cheap operation
    ///
    /// also it does not remove `contextual_validators`
    /// implicitly added when adding values, normally
    /// this should not be a problem, as validators are
    /// not supposed to error if the header they are meant
    /// for is not there or is there but has a different
    /// component type. Only if you remove a header and
    /// replace it by a alternate implementation of the
    /// same header which only differs in the contextual
    /// validator, in a way that  the new validator allows
    /// thinks the other does not _and_ you relay on it
    /// allowing this things it can cause an validation
    /// error.
    pub fn remove_by_name(&mut self, name: HeaderName ) -> bool {
        if let Some( HeaderBodies { first, other } ) = self.header_map.remove(&name) {
            //FIXME use HashSet once *mut T,T:?Sized impl Hash (rust #???)
            let mut to_remove_from_vec = Vec::<*const MailEncodable<E>>::new();
            to_remove_from_vec.push( first );
            if let Some( other ) = other {
                to_remove_from_vec.extend( other.into_iter().map(|x| x as *const _) )
            }

            self.header_vec.retain(|&(_name, ref boxed)| {
                let as_ptr = (&**boxed) as *const _;
                let keep_it = !to_remove_from_vec.contains(&as_ptr);
                keep_it
            });
            true
        } else {
            false
        }
    }
}


impl<E> fmt::Debug for HeaderMap<E>
    where E: MailEncoder
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        write!(fter, "HeaderMap {{ ")?;
        for (name, component) in self.iter() {
            write!(fter, "{}: {:?},", name, component)?;
        }
        write!(fter, " }}")
    }
}


pub struct UntypedMultiBodyIter<'a, E: 'a> {
    first: Option<&'a MailEncodable<E>>,
    other: Option<SliceIter<'a, *mut MailEncodable<E>>>,
}

impl<'a, E> UntypedMultiBodyIter<'a, E>
    where E: MailEncoder
{
    fn new(
        first: &'a MailEncodable<E>,
        other: Option<SliceIter<'a, *mut MailEncodable<E>>>
    ) -> Self {
        UntypedMultiBodyIter {
            first: Some(first),
            other: other,
        }
    }

    fn with_typing<H>(self) -> TypedMultiBodyIter<'a, E, H>
        where H: Header, H::Component: MailEncodable<E>
    {
        TypedMultiBodyIter {
            untyped_iter: self,
            _header_type: PhantomData
        }
    }
}

impl<'a, E> Iterator for UntypedMultiBodyIter<'a, E>
    where E: MailEncoder
{
    type Item = &'a MailEncodable<E>;

    fn next(&mut self) -> Option<Self::Item> {
        self.first
            .take()
            .or_else( || {
                self.other.as_mut()
                    .and_then( |other| other.next() )
                    //SAFE: all pointers in HeaderMap are always valid and
                    //  borrowing rules are indirectly enforced by borrowing
                    //  `&self` in `HeaderMap::get_untyped`
                    .map( |val| unsafe { &**val } )
            })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let offset = if self.first.is_some() { 1 } else { 0 };
        if let Some( other ) = self.other.as_ref() {
            let (min, max) = other.size_hint();
            (min+offset, max.map(|v|v+offset))
        } else {
            (offset, Some(offset))
        }
    }
}


pub struct TypedMultiBodyIter<'a, E: 'a, H> {
    untyped_iter: UntypedMultiBodyIter<'a, E>,
    _header_type: PhantomData<H>
}

impl<'a, E, H> Iterator for TypedMultiBodyIter<'a, E, H>
    where E: MailEncoder, H: Header, H::Component: MailEncodable<E>
{
    type Item = Result<&'a H::Component>;

    fn next(&mut self) -> Option<Self::Item> {
        let tobj_item = self.untyped_iter.next();
        tobj_item.map( |tobj| {
            tobj.downcast_ref::<H::Component>()
                .ok_or_else( ||->Error {
                    "use of different header types with same header name".into() } )
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.untyped_iter.size_hint()
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

    use self::collision_headers::{
        Subject as BadSubject,
        Comments as BadComments
    };

    mod collision_headers {
        use components;
        def_headers! {
            test_name: validate_header_names,
            scope: components,
            1 Subject, unsafe { "Subject" }, Mime,
            + Comments, unsafe { "Comments" }, Mime
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
        const CAN_APPEAR_MULTIPLE_TIMES: bool = true;
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
    fn insert_does_insert_validator() {
        let map = headers! {
            XComment: "yay",
            Subject: "soso"
        }.unwrap();
        typed(&map);

        assert_eq!(1, map.iter_contextual_validators().count());
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
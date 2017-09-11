use std::{ mem, fmt };
use std::any::{ Any, TypeId };
use std::marker::PhantomData;
use std::collections::{ HashMap as Map };
use std::iter::Iterator;
use std::slice::{ Iter as SliceIter };

//reexport for headers macro
pub use ascii::{ AsciiStr as _AsciiStr };

use utils::HeaderTryInto;
use error::*;
use codec::{ MailEncoder, MailEncodable };

use super::{
    HeaderName,
    Header,
    SingularHeaderMarker
};

mod into_iter;
pub use self::into_iter::*;
mod iter;
pub use self::iter::*;

//TODO implement: Debug,  remove
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
    header_vec: Vec<(HeaderName, *mut MailEncodable<E>)>,
    // we don't want to search for a header when accessing it,
    // we also have to kow if such a header is set etc.
    header_map: Map<HeaderName, HeaderBodies<E>>
}



struct HeaderBodies<E: MailEncoder> {
    first: Box<MailEncodable<E>>,
    other: Option<Vec<Box<MailEncodable<E>>>>
}


impl<E: MailEncoder> HeaderMap<E> {

    pub fn new() -> Self {
        HeaderMap {
            header_vec: Vec::new(),
            header_map: Map::new()
        }
    }

    ///
    /// Note:
    /// if you implement `SingularHeaderMarker` on a header
    /// which can appear multiple times this function will
    /// just return one of the multiple possible values
    /// (if there are any) with out any guarantees which one
    /// or that multiple call to it will always return the
    /// same one
    pub fn get_single<'a ,H>( &'a self ) -> Result<Option<&'a H::Component>>
        where H: Header + SingularHeaderMarker,
              H::Component: 'static
    {

      if let Some( body ) = self.get_bodies( H::name() ) {
            downcast_ref::<E, H::Component>( &*body.first )
                .ok_or_else( ||->Error {
                    "use of different header types with same header name".into() } )
                .map( |res_ref| Some( res_ref ) )
        } else {
            Ok( None )
        }

    }

    pub fn get<H>( &self ) -> Option<TypedMultiBodyIter<E, H>>
        where H: Header, H::Component: MailEncodable<E>
    {
        self.get_untyped( H::name() )
            .map( |untyped| untyped.with_typing() )
    }

    pub fn get_untyped( &self, name: HeaderName ) -> Option<UntypedMultiBodyIter<E>> {
        if let Some( body ) = self.get_bodies( name ) {
            Some( UntypedMultiBodyIter::new(
                &*body.first,
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

    // we can't have a public `std::iter::Extend` as insertion
    // is failable
    pub fn extend( &mut self, other: HeaderMap<E> ) -> Result<()> {
        let HeaderMap { header_vec, header_map } = other;
        //SAFETY: after dropping header_vec using header_map in any way
        // is completely safe
        mem::drop(header_vec);
        for (name, hbody) in header_map.into_iter() {
            self.insert_trait_object( name, hbody.first, hbody.other.is_some())?;
            if let Some( other ) = hbody.other {
                for tobj in other {
                    self.insert_trait_object( name, tobj, true )?;
                }
            }
        }
        Ok( () )
    }

    ///
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
        let tobj: Box<MailEncodable<E>> = Box::new( hbody );
        self.insert_trait_object( H::name(), tobj, H::CAN_APPEAR_MULTIPLE_TIMES )
    }

    //FIXME make can_appear_multiple_times a constant generic parameter
    // when supported by rust
    fn insert_trait_object(
        &mut self,
        name: HeaderName,
        mut tobj: Box<MailEncodable<E>>,
        can_appear_multiple_times: bool
    ) -> Result<()> {
        //SAFTY: we get a second pointer, while using two at a time is unsafe
        //  the mere existence is not a problem

        let obj_ptr = (&mut *tobj) as *mut MailEncodable<E>;
        self._insert_trait_object_to_map(name, tobj, can_appear_multiple_times)?;
        //only if we succesfull inserted it to the map can we insert it to the vec
        self.header_vec.push( (name, obj_ptr) );
        Ok( () )
    }

    fn _insert_trait_object_to_map(
        &mut self,
        name: HeaderName,
        obj: Box<MailEncodable<E>>,
        can_appear_multiple_times: bool
    ) -> Result<()> {
        {
            if let Some( body ) = self.header_map.get_mut( &name ) {
                if !can_appear_multiple_times {
                    bail!( "field already set and field can appear at most one time" );
                }
                if let Some( other ) = body.other.as_mut() {
                    other.push( obj );
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
            first: obj,
            other: empty_other
        } );

        Ok( () )
    }

    // we currently do not have a mechanism to remove header
//    fn _remove_from_vec( &mut self, obj_ptr: *mut MailEncodable<E> ) {
//        let ptr_as_num = obj_ptr as usize;
//        let mut rem_idx = None;
//        for (idx, &(name, ptr)) in self.header_map.iter().enumerate() {
//            if ptr as usize == ptr_as_num {
//                rem_idx = Some(idx)
//            }
//        }
//        if let Some( rem_idx ) = rem_idx {
//            self.header_vec.remove( rem_idx );
//        } else {
//            panic!(concat!(
//                "no matching ptr found in vec ==",
//                " inconsistent state ==",
//                " possible broken safety gurantees",
//                " (or just misuse of _remove_from_vec fn)"
//            ));
//        }
//    }
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
    other: Option<SliceIter<'a, Box<MailEncodable<E>>>>,
}

impl<'a, E> UntypedMultiBodyIter<'a, E>
    where E: MailEncoder
{
    fn new(
        first: &'a MailEncodable<E>,
        other: Option<SliceIter<'a, Box<MailEncodable<E>>>>
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
                    .map( |val| &**val )
            })
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
        ContentType, Subject, From
    };
    use super::*;

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
            .get::<ContentType>()
            .expect( "content type header must be present" )
            .map( |h: Result<&Mime>| {
                // each of the multiple values could have a different
                // type then H::Component
                h.expect( "the trait object to be downcastable to H::Component" );
            })
            .count();
        assert_eq!( 1, count );

        let count = headers
            .get::<Subject>()
            .expect( "content type header must be present" )
            .map( |h: Result<&Unstructured>| {
                h.expect( "the trait object to be downcastable to H::Component" );
            })
            .count();
        assert_eq!( 1, count );

        let count = headers
            .get::<From>()
            .expect( "content type header must be present" )
            .map( |h: Result<&MailboxList>| {
                h.expect( "the trait object to be downcastable to H::Component" );
            })
            .count();
        assert_eq!( 1, count );

        typed(&headers);
    }

    #[test]
    fn iter() {

    }

    #[test]
    fn get_single() {

    }

    #[test]
    fn get() {

    }

    #[test]
    fn fmt_debug() {
        use headers::Subject;

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
}
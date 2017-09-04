use std::any::{ Any, TypeId };
use std::collections::{ HashMap as Map };
use std::marker::PhantomData;
use std::slice::{ Iter as SliceIter };
use std::collections::hash_map::{
    Iter as MapIter,
    IterMut as MapIterMut,
    IntoIter as MapIntoIter,
    Entry
};
use std::iter::{ Iterator, Extend };

use ascii::AsciiStr;
pub use ascii::{ AsciiStr as _AsciiStr };

use utils::HeaderTryInto;
use error::*;
use grammar::is_ftext;
use codec::{ MailEncoder, MailEncodable };
use components::{ Mime };

use super::default_header::{ ContentType, ContentTransferEncoding };
use super::{ HeaderName, Header, SingularHeaderMarker };

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
    headers: Map<HeaderName, HeaderBodies<E>>,
}


struct HeaderBodies<E: MailEncoder> {
    first: Box<MailEncodable<E>>,
    other: Option<Vec<Box<MailEncodable<E>>>>
}


impl<E: MailEncoder> HeaderMap<E> {

    pub fn new() -> Self {
        HeaderMap { headers: Map::new() }
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
        where H: Header, H::Component: MailEncodable<E>
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

    // we can't have a public `std::iter::Extend` as insertion
    // is failable
    pub fn extend( &mut self, other: HeaderMap<E> ) -> Result<()> {
        for (name, hbody) in other.headers.into_iter() {
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
        tobj: Box<MailEncodable<E>>,
        can_appear_multiple_times: bool
    ) -> Result<()> {
        {
            if let Some( body ) = self.headers.get_mut( &name ) {
                let has_multiple = body.other.is_some();
                if can_appear_multiple_times != has_multiple {
                    bail!( "multi appearance header combined with single apparence header with same name" );
                }
                if can_appear_multiple_times {
                    //UNWRAP_SAFE: as multi == has_multi == other.is_some() == true
                    body.other.as_mut().unwrap().push( tobj );
                } else {
                    //override non multi entry
                    body.first = tobj;
                }
                return Ok(())
            }
        }

        let empty_other = if can_appear_multiple_times {
            Some( Vec::new() )
        } else {
            None
        };

        self.headers.insert( name.to_owned(), HeaderBodies {
            first: tobj,
            other: empty_other
        });

        Ok( () )
    }
}



pub struct HeaderMultiBodyIter<'a, E: 'a, H> {
    first: Option<&'a MailEncodable<E>>,
    other: Option<SliceIter<'a, Box<MailEncodable<E>>>>,
    _header_type: PhantomData<H>
}

impl<'a, E, H> HeaderMultiBodyIter<'a, E, H>
    where E: MailEncoder, H: Header, H::Component: MailEncodable<E>
{
    fn new(
        first: &'a MailEncodable<E>,
        other: Option<SliceIter<'a, Box<MailEncodable<E>>>>
    ) -> Self {
        HeaderMultiBodyIter {
            first: Some(first),
            other: other,
            _header_type: PhantomData
        }
    }
}


impl<'a, E, H> Iterator for HeaderMultiBodyIter<'a, E, H>
    where E: MailEncoder, H: Header, H::Component: MailEncodable<E>
{
    type Item = Result<&'a H::Component>;

    fn next(&mut self) -> Option<Self::Item> {
        let tobj_item = self.first
            .take()
            .or_else( || {
                self.other.as_mut()
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
        Mime, Unstructured, Mailbox,
        MailboxList
    };
    use headers::{
        ContentType, Subject, From
    };
    use super::*;

    fn typed(v: &HeaderMap<MailEncoderImpl>) {}

    #[test]
    fn headers_macro() {
        let headers = headers! {
            ContentType: "text/plain; charset=us-ascii",
            Subject: "Having a lot of fun",
            From: ("Bla Blup", "bla.blub@not.a.domain")
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
}
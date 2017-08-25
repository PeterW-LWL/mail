use std::path::PathBuf;
use std::fmt;
use std::sync::{ Arc, RwLock, RwLockWriteGuard, RwLockReadGuard };
use std::ops::Deref;
use std::mem;
use std::borrow::Cow;

use mime::Mime;

use futures::future::BoxFuture;
use futures::{  Future, Poll, Async };

use error::{ Error, Result };

use codec::transfer_encoding::TransferEncodedFileBuffer;
use components::TransferEncoding;


use mime::TEXT_PLAIN;
use utils::FileBuffer;
use super::BuilderContext;


/// POD containing the path from which the resource should be loaded as well as mime and name
/// if no mime is specified, the mime is sniffed if possible
/// if no name is specified the base name of the path is used
#[derive( Debug, Clone )]
pub struct ResourceSpec {
    pub path: PathBuf,
    pub use_name: Option<PathBuf>,
    pub use_mime: Option<Mime>
}

#[derive(Debug)]
pub struct ResourceFutureRef<'a, C: 'a> {
    resource_ref: &'a mut Resource,
    ctx_ref: &'a C
}

#[derive( Debug, Clone )]
pub struct Resource {
    inner: Arc<RwLock<ResourceInner>>,
    preferred_encoding: Option<TransferEncoding>
}


enum ResourceInner {
    Spec( ResourceSpec ),
    LoadingBuffer( BoxFuture<FileBuffer, Error> ),
    Loaded( FileBuffer ),
    EncodingBuffer( BoxFuture<TransferEncodedFileBuffer, Error> ),
    TransferEncoded( TransferEncodedFileBuffer ),
    Failed
}

pub struct Guard<'lock> {
    //NOTE: this is NOT dead_code, just unused through it still _drops_
    #[allow(dead_code)]
    guard: RwLockReadGuard<'lock, ResourceInner>,
    inner_ref: *const TransferEncodedFileBuffer
}



impl Resource {

    pub fn from_text(text: String ) -> Self {
        //UNWRAP_SAFE: this is a valid mime, if not this will be coucht by the tests
        let mime: Mime = "text/plain;charset=utf8".parse().unwrap();
        let buf = FileBuffer::new( mime, text.into_bytes() );
        Resource::from_buffer( buf )
    }
    #[inline]
    pub fn from_spec( spec: ResourceSpec ) -> Self {
        Self::new_inner( ResourceInner::Spec( spec ) )
    }

    #[inline]
    pub fn from_buffer( buffer: FileBuffer ) -> Self {
        Self::new_inner( ResourceInner::Loaded( buffer ) )
    }

    #[inline]
    pub fn from_future( fut: BoxFuture<FileBuffer, Error> ) -> Self {
        Self::new_inner( ResourceInner::LoadingBuffer( fut ) )
    }

    #[inline]
    pub fn from_encoded_buffer( buffer: TransferEncodedFileBuffer ) -> Self {
        Self::new_inner( ResourceInner::TransferEncoded( buffer ) )
    }

    #[inline]
    pub fn from_future_encoded( fut: BoxFuture<TransferEncodedFileBuffer, Error> ) -> Self {
        Self::new_inner( ResourceInner::EncodingBuffer( fut ) )
    }


    pub fn set_preferred_encoding( &mut self, tenc: TransferEncoding ) {
        self.preferred_encoding = Some( tenc )
    }

    fn new_inner( r: ResourceInner ) -> Self {
        Resource {
            inner: Arc::new( RwLock::new( r ) ),
            preferred_encoding: None
        }
    }

    fn read_inner( &self ) -> Result<RwLockReadGuard<ResourceInner>> {
        match self.inner.read() {
            Ok( guard ) => Ok( guard ),
            Err( .. ) => bail!( "[BUG] lock was poisoned" )
        }
    }

    fn write_inner( &self ) -> Result<RwLockWriteGuard<ResourceInner>> {
        match self.inner.write() {
            Ok( guard ) => Ok( guard ),
            Err( .. ) => bail!( "[BUG] lock was poisoned" )
        }
    }

    pub fn get_if_encoded( &self ) -> Result<Option<Guard>> {
        use self::ResourceInner::*;
        let inner = self.read_inner()?;
        let ptr = match *inner {
            TransferEncoded( ref encoded )  => Some( encoded as *const TransferEncodedFileBuffer ),
            _ => None
        };

        Ok( ptr.map( |ptr |Guard {
            guard: inner,
            inner_ref: ptr,
        } ) )
    }

    pub fn as_future<'a, C>( &'a mut self, ctx: &'a C ) -> ResourceFutureRef<'a, C> {
        ResourceFutureRef {
            resource_ref: self,
            ctx_ref: ctx
        }
    }

    pub fn poll_encoding_completion<C>( &mut self, ctx: &C ) -> Poll<(), Error>
        where C: BuilderContext
    {
        let mut inner = self.write_inner()?;
        let moved_out = mem::replace( &mut *inner, ResourceInner::Failed );
        let (move_back_in, state) =
            Resource::_poll_encoding_completion( moved_out, ctx, &self.preferred_encoding )?;
        mem::replace( &mut *inner, move_back_in );
        Ok( state )

    }

    fn _poll_encoding_completion<C>(
        resource: ResourceInner,
        ctx: &C,
        pref_enc: &Option<TransferEncoding>
    ) -> Result<(ResourceInner, Async<()>)>
        where C: BuilderContext
    {
        use self::ResourceInner::*;
        let mut continue_with = resource;
        // NOTE(why the loop):
        // we only return if we polled on a contained future and it resulted in
        // `Async::NotReady` or if we return `Async::Ready`. If we would not do
        // so the Spawn(/Run?/Task?) might think we are waiting for something _external_
        // and **park** the task e.g. by masking it as not ready in tokio or calling
        // `thread::park()` in context of `Future::wait`.
        //
        // Alternatively we also could call `task::current().notify()` in all
        // cases where we would return a `NotReady` from our side (e.g.
        // when we got a ready from file loading and advance the to `Loaded` )
        // but using a loop here should be better.
        loop {
            continue_with = match continue_with {
                Spec(spec) => {
                    let ResourceSpec { path, use_mime, use_name } = spec;
                    LoadingBuffer(
                        ctx.execute( ctx.load_file( Cow::Owned( path ) ).map( move |data| {
                            //FIXME actually use use_name!
                            let _ = use_name;
                            //if let spec.name => buf.file_meta_mut().file_name = Some( name )
                            //FIXME actually sniff mime is use_mime is none
                            FileBuffer::new( use_mime.unwrap(), data )
                        } ) )
                    )
                },

                LoadingBuffer(mut fut) => {
                    match fut.poll()? {
                        Async::Ready( buf )=> Loaded( buf ),
                        Async::NotReady => {
                            return Ok( ( LoadingBuffer(fut), Async::NotReady ) )
                        }
                    }
                },

                Loaded(buf) => {
                    let pe = pref_enc.clone();
                    EncodingBuffer( ctx.execute_fn(move || {
                        TransferEncodedFileBuffer::encode_buffer(buf, pe.as_ref())
                    } ) )
                },

                EncodingBuffer(mut fut) => {
                    match fut.poll()? {
                        Async::Ready( buf )=> TransferEncoded( buf ),
                        Async::NotReady => {
                            return Ok( ( EncodingBuffer(fut), Async::NotReady ) )
                        }
                    }
                },

                ec @ TransferEncoded(..) => {
                    return Ok( ( ec , Async::Ready( () ) ) )
                },

                Failed => {
                    bail!( "failed already in previous poll" );
                }
            }
        }
    }


    /// mainly for testing
    pub fn empty_text() -> Self {
        Resource::from_buffer( FileBuffer::new( TEXT_PLAIN, Vec::new() ) )
    }

}


impl<'a, C: 'a> Future for ResourceFutureRef<'a, C>
    where C: BuilderContext
{
    type Item = ();
    type Error = Error;

    fn poll( &mut self ) -> Poll<Self::Item, Self::Error> {
        self.resource_ref.poll_encoding_completion( self.ctx_ref )
    }
}



impl fmt::Debug for ResourceInner {
    fn fmt( &self, fter: &mut fmt::Formatter ) -> fmt::Result {
        use self::ResourceInner::*;
        match *self {
            Spec( ref spec ) => <ResourceSpec as fmt::Debug>::fmt( spec, fter ),
            LoadingBuffer( .. ) => write!( fter, "LoadingBuffer( future )" ),
            Loaded( ref buf ) => <FileBuffer as fmt::Debug>::fmt( buf, fter ),
            EncodingBuffer( .. ) => write!( fter, "EncodingBuffer( future )" ),
            TransferEncoded( ref buf ) => <TransferEncodedFileBuffer as fmt::Debug>::fmt( buf, fter ),
            Failed => write!( fter, "Failed" )
        }
    }
}


impl<'a> Deref for Guard<'a> {
    type Target = TransferEncodedFileBuffer;

    fn deref( &self ) -> &TransferEncodedFileBuffer {
        //SAFE: the lifetime of the value behind the inner_ref pointer is bound
        // to the lifetime of the RwLock and therefore lives longer as
        // the Guard which is also part of this struct and therefore
        // has to life at last as long as the struct
        unsafe { &*self.inner_ref }
    }
}

//TODO make test require default impl
#[cfg(test)]
mod test {
    use std::fmt::Debug;
    use futures::Future;
    use futures::future::Either;

    use super::*;

    use default_impl::SimpleContext;
    use default_impl::VFSFileLoader;

    use utils::timeout;

    fn resolve_resource<C: BuilderContext+Debug>( resource: &mut Resource, ctx: &C ) {
        let res = resource
            .as_future( ctx )
            .select2( timeout( 1, 0 ) )
            .wait()
            .unwrap();

        match res {
            Either::A( .. ) => { },
            Either::B( .. ) => {
                panic!( "timeout! resource as future did never resolve to either Item/Error" )
            }
        }
    }

    #[test]
    fn load_test() {
        let mut fload = VFSFileLoader::new();
        fload.register_file( "/test/me.yes", b"abc def!".to_vec() ).unwrap();
        let ctx = SimpleContext::with_vfs( "test.notadomain".into(), fload );

        let spec = ResourceSpec {
            path: "/test/me.yes".into(),
            use_name: None,
            use_mime: Some( "text/plain;charset=us-ascii".parse().unwrap() ) 
        };

        let mut resource = Resource::from_spec( spec );

        assert_eq!( false, resource.get_if_encoded().unwrap().is_some() );

        resolve_resource( &mut resource, &ctx );

        let res = resource.get_if_encoded().unwrap().unwrap();
        let enc_buf: &TransferEncodedFileBuffer = &*res;
        let data: &[u8] = &*enc_buf;
        
        assert_eq!( b"abc def!", data );
    }


    #[test]
    fn load_test_utf8() {
        let mut fload = VFSFileLoader::new();
        fload.register_file( "/test/me.yes", "Öse".as_bytes().to_vec() ).unwrap();
        let ctx = SimpleContext::with_vfs( "test.notadomain".into(), fload );

        let spec = ResourceSpec {
            path: "/test/me.yes".into(),
            use_name: None,
            use_mime: Some( "text/plain;charset=utf8".parse().unwrap() )
        };

        let mut resource = Resource::from_spec( spec );

        assert_eq!( false, resource.get_if_encoded().unwrap().is_some() );

        resolve_resource( &mut resource, &ctx );

        let res = resource.get_if_encoded().unwrap().unwrap();
        let enc_buf: &TransferEncodedFileBuffer = &*res;
        let data: &[u8] = &*enc_buf;

        assert_eq!( b"=C3=96se", data );
    }

    #[ignore]
    #[test]
    fn test_use_name() {
        unimplemented!();
    }

    #[ignore]
    #[test]
    fn test_sniff_mime() {
        unimplemented!();
    }


    #[test]
    fn from_text_works() {
        let mut resource = Resource::from_text( "orange juice".into() );
        resolve_resource( &mut resource, &SimpleContext::new( "random".into() ) );
        let res = resource.get_if_encoded().unwrap().unwrap();
        let data: &[u8] = &*res;
        assert_eq!( b"orange juice", data );
    }




}
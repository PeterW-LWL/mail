use futures::Future;
use std::marker::Send;
use std::any::TypeId;
use std::cell::RefCell;
use std::mem;
use std::fmt::{self, Debug};

#[cfg(test)]
use futures::sync::oneshot;
#[cfg(test)]
use std::time::Duration;
#[cfg(test)]
use std::thread;

use mime::{ Mime, MULTIPART };
use error::Error;

mod buffer;
pub use self::buffer::FileBuffer;

mod date_time;
pub use self::date_time::DateTime;

mod file_meta;
pub use self::file_meta::FileMeta;


pub struct DebugIterableOpaque<I> {
    one_use_inner: RefCell<I>
}

impl<I> DebugIterableOpaque<I> {
    pub fn new(one_use_inner: I) -> Self {
        let one_use_inner = RefCell::new(one_use_inner);
        DebugIterableOpaque { one_use_inner }
    }
}
impl<I> Debug for DebugIterableOpaque<I>
    where I: Iterator, I::Item: Debug
{
    fn fmt(&self, fter: &mut fmt::Formatter) -> fmt::Result {
        let mut borrow = self.one_use_inner.borrow_mut();
        fter.debug_list().entries(&mut *borrow).finish()
    }
}


pub fn is_multipart_mime( mime: &Mime ) -> bool {
    mime.type_() == MULTIPART
}


#[cfg(test)]
pub(crate) fn timeout( s: u32, ms: u32 ) -> oneshot::Receiver<()> {
    let (timeout_trigger, timeout) = oneshot::channel::<()>();

    thread::spawn( move || {
        thread::sleep( Duration::new( s as u64, ms * 1_000_000) );
        //we do not care if it faile i.e. the receiver got dropped
        let _ = timeout_trigger.send( () );
    });

    timeout
}


//modified, origin is:
// https://github.com/rust-lang/rust/blob/2fbba5bdbadeef403a64e9e1568cdad225cbcec1/src/liballoc/string.rs
pub(crate) fn insert_bytes(vec: &mut Vec<u8> , idx: usize, bytes: &[u8]) {
    use std::ptr;
    let len = vec.len();
    let amount = bytes.len();
    vec.reserve(amount);

    unsafe  {
        ptr::copy( vec.as_ptr().offset( idx as isize ),
                   vec.as_mut_ptr().offset( (idx + amount) as isize ),
                   len - idx );
        ptr::copy( bytes.as_ptr(),
                   vec.as_mut_ptr().offset( idx as isize ),
                   amount );

        vec.set_len( len + amount );
    }
}


pub fn unbox_ref<E: ?Sized>( bxr: &Box<E> ) -> &E {
    &**bxr
}
pub fn unbox_mut<E: ?Sized>( bxm: &mut Box<E> ) -> &mut E {
    &mut **bxm
}


//TODO replace with std TryFrom once it is stable
// (either a hard replace, or a soft replace which implements HeaderTryFrom if TryFrom exist)
pub trait HeaderTryFrom<T>: Sized {
    fn try_from(val: T) -> Result<Self, Error>;
}

pub trait HeaderTryInto<T>: Sized {
    fn try_into(self) -> Result<T, Error>;
}

impl<F, T> HeaderTryInto<T> for F where T: HeaderTryFrom<F> {
    fn try_into(self) -> Result<T, Error> {
        T::try_from(self)
    }
}

// if the HeaderTryFrom chain is again liked with the From chain this
// must be removed as it will be carried over by the from chain
impl<T> HeaderTryFrom<T> for T {
    fn try_from(val: T) -> Result<Self, Error> {
        Ok( val )
    }
}

//FOR some reason if this is there HeaderTryFrom<&str> for Mime wont work
// EVEN THROUGH mime does not has any other From/Into/HeaderTryFrom/HeaderTryInto
// implementaion
//impl<T,F> HeaderTryFrom<F> for T where F: Into<T> {
//    fn try_from(val: F) -> Result<T, Error> {
//        Ok( val.into() )
//    }
//}




//FIXME: make it ?Sized once it's supported by rust
///
/// Used to undo type erasure in a generic context,
/// roughly semantically eqivalent to creating a `&Any`
/// type object from the input and then using `downcast_ref::<EXP>()`,
/// except that it does not require the cration of a
/// trait object as a step inbetween.
///
/// Note:
/// This function can be used for some form of specialisation,
/// (not just in a performence sense) but all "specialization path"
/// have to be known when writing the unspeciallized version and
/// it is easy to make functions behave in a unexpected (but safe)
/// way so use with care.
///
///
#[inline(always)]
pub fn uneraser_ref<GOT: 'static, EXP: 'static>(inp: &GOT ) -> Option<&EXP>  {
    if TypeId::of::<GOT>() == TypeId::of::<EXP>() {
        //SAFE: the GOT type is exact the same as the EXP type,
        // the compiler just does not know this due to type erasure wrt.
        // generic types
        let res: &EXP = unsafe { mem::transmute::<&GOT, &EXP>(inp) };
        Some( res )
    } else {
        None
    }
}

//FIXME: make it ?Sized once it's supported by rust
#[inline(always)]
pub fn uneraser_mut<GOT: 'static, EXP: 'static>(inp: &mut GOT ) -> Option<&mut EXP> {
    if TypeId::of::<GOT>() == TypeId::of::<EXP>() {
        //SAFE: the GOT type is exact the same as the EXP type,
        // the compiler just does not know this due to type erasure wrt.
        // generic types
        let res: &mut EXP = unsafe { mem::transmute::<&mut GOT, &mut EXP>(inp) };
        Some( res )
    } else {
        None
    }
}

//FIXME: only works if the rust compiler get's a bit more clever or a bit less (either is fine)
//#[inline(always)]
//pub fn uneraser<GOT: 'static, EXP: 'static>( inp: GOT ) -> Result<EXP, GOT> {
//    if TypeId::of::<GOT>() == TypeId::of::<EXP>() {
//        //SAFE: the GOT type is exact the same as the EXP type,
//        // the compiler just does not know this due to type erasure wrt.
//        // generic types
//        Ok( unsafe { mem::transmute::<GOT, EXP>( inp ) } )
//    } else {
//        Err( inp )
//    }
//}

//fn get_flat_byte_repr<T>(val: &T) -> Vec<u8> {
//    let count = mem::size_of::<T>();
//    let mut out = Vec::with_capacity(count);
//    let byte_ptr = val as *const T as *const u8;
//    for offset in 0..count {
//        out.push( unsafe {
//            *byte_ptr.offset(offset as isize)
//        })
//    }
//    out
//}

pub type SendBoxFuture<I, E> = Box<Future<Item=I, Error=E>+Send>;



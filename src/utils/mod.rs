#[cfg(test)]
use futures::sync::oneshot;
#[cfg(test)]
use std::time::Duration;
#[cfg(test)]
use std::thread;

use mime::Mime;
use mime::MULTIPART;



mod buffer;
pub use self::buffer::FileBuffer;

mod date_time;
pub use self::date_time::DateTime;

mod file_meta;
pub use self::file_meta::FileMeta;

#[macro_use]
mod vec1;
pub use self::vec1::Vec1;


pub fn is_multipart_mime( mime: &Mime ) -> bool {
    mime.type_() == MULTIPART
}


#[cfg(test)]
pub(crate) fn timeout( s: u32, ms: u32 ) -> oneshot::Receiver<()> {
    let (timeout_trigger, timeout) = oneshot::channel::<()>();

    thread::spawn( move || {
        thread::sleep( Duration::new( s as u64, ms * 1_000_000) );
        timeout_trigger.send( () ).unwrap()
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






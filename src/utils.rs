use futures::Future;
use std::marker::Send;

pub type SendBoxFuture<I, E> = Box<Future<Item=I, Error=E>+Send>;


#[cfg(test)]
use futures::sync::oneshot;
#[cfg(test)]
use std::time::Duration;
#[cfg(test)]
use std::thread;


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
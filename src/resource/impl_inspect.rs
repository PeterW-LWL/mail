use super::{InspectEmbeddedResources, Embedded};

use std::collections::{VecDeque, LinkedList, HashMap, BTreeMap};
use std::hash::Hash;
use std::ops::{Deref, DerefMut};

use std::sync::Arc;
use std::rc::Rc;
use std::path::{Path, PathBuf};
use std::ffi::{OsStr, OsString};
use std::net::*;
use std::env::*;
use std::io::*;

macro_rules! impl_leaf_simple_maybe_sized {
    ($($name:ident),*,) => (impl_leaf_unsized!($($name),*););
    ($($name:ident),*) => ($(
        impl InspectEmbeddedResources for $name {
            fn inspect_resources(&self, _visitor: &mut FnMut(&Embedded)) {
                //nop
            }
            fn inspect_resources_mut(&mut self, _visitor: &mut FnMut(&mut Embedded)) {
                //nop
            }
        }

        impl<'a> InspectEmbeddedResources for &'a $name {
            fn inspect_resources(&self, _visitor: &mut FnMut(&Embedded)) {
                //nop
            }
            fn inspect_resources_mut(&mut self, _visitor: &mut FnMut(&mut Embedded)) {
                //nop
            }
        }

        impl InspectEmbeddedResources for Arc<$name> {
            fn inspect_resources(&self, _visitor: &mut FnMut(&Embedded)) {
                //nop
            }
            fn inspect_resources_mut(&mut self, _visitor: &mut FnMut(&mut Embedded)) {
                //nop
            }
        }

        impl InspectEmbeddedResources for Rc<$name> {
            fn inspect_resources(&self, _visitor: &mut FnMut(&Embedded)) {
                //nop
            }
            fn inspect_resources_mut(&mut self, _visitor: &mut FnMut(&mut Embedded)) {
                //nop
            }
        }
    )*);
}

impl_leaf_simple_maybe_sized! {
    str, Path, OsStr
}

macro_rules! impl_leaf_simple_sized {
    ($($name:ident),*,) => (impl_leaf_simple!($($name),*););
    ($($name:ident),*) => ($(
        impl_leaf_simple_maybe_sized!{ $name }


        impl<'a> InspectEmbeddedResources for &'a [$name] {
            fn inspect_resources(&self, _visitor: &mut FnMut(&Embedded)) {
                //nop
            }
            fn inspect_resources_mut(&mut self, _visitor: &mut FnMut(&mut Embedded)) {
                //nop
            }
        }
    )*);
}

impl_leaf_simple_sized! {
    u8, i8, u16, i16, u32, i32, u128, i128,
    f32, f64, bool, char,
    String, PathBuf, OsString,

    //net
    AddrParseError, Ipv4Addr, Ipv6Addr,
    SocketAddrV4, SocketAddrV6, TcpListener,
    TcpStream, UdpSocket,
    IpAddr, Shutdown, SocketAddr,

    //env
    Args,
    ArgsOs,
    JoinPathsError,
    Vars,
    VarsOs,
    VarError,


    //io
    Empty,
    Error,
    Repeat,
    Sink,
    Stderr,
    Stdin,
    Stdout,
    ErrorKind,
    SeekFrom
    //TODO more of std
}

impl<T> InspectEmbeddedResources for [T]
    where T: InspectEmbeddedResources
{
    fn inspect_resources(&self, visitor: &mut FnMut(&Embedded)) {
        for item in self.iter() {
            item.inspect_resources(visitor)
        }
    }
    fn inspect_resources_mut(&mut self, visitor: &mut FnMut(&mut Embedded)) {
        for item in self.iter_mut() {
            item.inspect_resources_mut(visitor)
        }
    }
}

impl<'a, T> InspectEmbeddedResources for &'a mut T
    where T: InspectEmbeddedResources
{
    fn inspect_resources(&self, visitor: &mut FnMut(&Embedded)) {
        (**self).inspect_resources(visitor)
    }
    fn inspect_resources_mut(&mut self, visitor: &mut FnMut(&mut Embedded)) {
        (**self).inspect_resources_mut(visitor)
    }
}

macro_rules! impl_seq_like {
    ($($name:ident),*) => ($(
        impl<T> InspectEmbeddedResources for $name<T>
            where T: InspectEmbeddedResources
        {
            fn inspect_resources(&self, visitor: &mut FnMut(&Embedded)) {
                for item in self.iter() {
                    item.inspect_resources(visitor)
                }
            }

            fn inspect_resources_mut(&mut self, visitor: &mut FnMut(&mut Embedded)) {
                for item in self.iter_mut() {
                    item.inspect_resources_mut(visitor)
                }
            }
        }
    )*);
}

impl_seq_like!(Vec, VecDeque, LinkedList);

macro_rules! impl_map_like {
    ($($name:ident [$($c:tt)*]),*) => ($(
        impl<K, T> InspectEmbeddedResources for $name<K, T>
            where T: InspectEmbeddedResources, K: $($c)*
        {
            fn inspect_resources(&self, visitor: &mut FnMut(&Embedded)) {
                for item in self.values() {
                    item.inspect_resources(visitor)
                }
            }

            fn inspect_resources_mut(&mut self, visitor: &mut FnMut(&mut Embedded)) {
                for item in self.values_mut() {
                    item.inspect_resources_mut(visitor)
                }
            }
        }
    )*);
}

impl_map_like!(HashMap [Eq + Hash], BTreeMap [Ord]);


macro_rules! impl_deref_like {
    ($($name:ident),*) => ($(
        impl<T> InspectEmbeddedResources for $name<T>
            where T: InspectEmbeddedResources
        {
            fn inspect_resources(&self, visitor: &mut FnMut(&Embedded)) {
                self.deref().inspect_resources(visitor)
            }

            fn inspect_resources_mut(&mut self, visitor: &mut FnMut(&mut Embedded)) {
                self.deref_mut().inspect_resources_mut(visitor)
            }
        }
    )*);
}

impl_deref_like!(Box);

impl<T> InspectEmbeddedResources for Option<T>
    where T: InspectEmbeddedResources
{
    fn inspect_resources(&self, visitor: &mut FnMut(&Embedded)) {
        if let Some(val) = self.as_ref() {
            val.inspect_resources(visitor)
        }
    }

    fn inspect_resources_mut(&mut self, visitor: &mut FnMut(&mut Embedded)) {
        if let Some(val) = self.as_mut() {
            val.inspect_resources_mut(visitor)
        }
    }
}


#[cfg(test)]
mod test {

    mod simple_derive_and_inspect {
        use mail::Resource;
        use ::resource::{InspectEmbeddedResources, Embedded};

        #[derive(InspectEmbeddedResources)]
        struct SomeTestStruct<'a> {
            count: u32,
            logo: Option<Embedded>,
            ref_logo: &'a mut Option<Embedded>,
            ref_logo2: Option<&'a mut  Embedded>,

            #[mail(inspect_skip)]
            no_auto_impl: Option<&'a Embedded>
        }

        #[test]
        fn compiled_now_try_it_1() {
            let mut logo = None;
            let mut instance = SomeTestStruct {
                count: 12,
                logo: None,
                ref_logo: &mut logo,
                ref_logo2: None,
                no_auto_impl: None
            };

            let mut counter = 0;

            instance.inspect_resources(&mut |_| counter += 1);
            instance.inspect_resources_mut(&mut |_| counter += 1);

            assert_eq!(counter, 0)
        }

        fn any_embedded() -> Embedded {
            Embedded::attachment(Resource::sourceless_from_string("abc"))
        }

        #[test]
        fn compiled_now_try_it_2() {
            let mut emb = any_embedded();
            let emb2 = any_embedded();
            let mut logo = Some(any_embedded());
            let mut instance = SomeTestStruct {
                count: 12,
                logo: Some(any_embedded()),
                ref_logo: &mut logo,
                ref_logo2: Some(&mut emb),
                no_auto_impl: Some(& emb2)
            };

            let mut counter = 0;

            instance.inspect_resources(&mut |_| counter += 1);
            instance.inspect_resources_mut(&mut |_| counter += 1);

            assert_eq!(counter, 6)
        }
    }
}
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
            fn inspect_resources(&self, _visitor: &mut impl FnMut(&Embedded)) {
                //nop
            }
            fn inspect_resources_mut(&mut self, _visitor: &mut impl FnMut(&mut Embedded)) {
                //nop
            }
        }

        impl<'a> InspectEmbeddedResources for &'a $name {
            fn inspect_resources(&self, _visitor: &mut impl FnMut(&Embedded)) {
                //nop
            }
            fn inspect_resources_mut(&mut self, _visitor: &mut impl FnMut(&mut Embedded)) {
                //nop
            }
        }

        impl InspectEmbeddedResources for Arc<$name> {
            fn inspect_resources(&self, _visitor: &mut impl FnMut(&Embedded)) {
                //nop
            }
            fn inspect_resources_mut(&mut self, _visitor: &mut impl FnMut(&mut Embedded)) {
                //nop
            }
        }

        impl InspectEmbeddedResources for Rc<$name> {
            fn inspect_resources(&self, _visitor: &mut impl FnMut(&Embedded)) {
                //nop
            }
            fn inspect_resources_mut(&mut self, _visitor: &mut impl FnMut(&mut Embedded)) {
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
            fn inspect_resources(&self, _visitor: &mut impl FnMut(&Embedded)) {
                //nop
            }
            fn inspect_resources_mut(&mut self, _visitor: &mut impl FnMut(&mut Embedded)) {
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
    fn inspect_resources(&self, visitor: &mut impl FnMut(&Embedded)) {
        for item in self.iter() {
            item.inspect_resources(visitor)
        }
    }
    fn inspect_resources_mut(&mut self, visitor: &mut impl FnMut(&mut Embedded)) {
        for item in self.iter_mut() {
            item.inspect_resources_mut(visitor)
        }
    }
}

impl<'a, T> InspectEmbeddedResources for &'a mut T
    where T: InspectEmbeddedResources
{
    fn inspect_resources(&self, visitor: &mut impl FnMut(&Embedded)) {
        (**self).inspect_resources(visitor)
    }
    fn inspect_resources_mut(&mut self, visitor: &mut impl FnMut(&mut Embedded)) {
        (**self).inspect_resources_mut(visitor)
    }
}

macro_rules! impl_seq_like {
    ($($name:ident),*) => ($(
        impl<T> InspectEmbeddedResources for $name<T>
            where T: InspectEmbeddedResources
        {
            fn inspect_resources(&self, visitor: &mut impl FnMut(&Embedded)) {
                for item in self.iter() {
                    item.inspect_resources(visitor)
                }
            }

            fn inspect_resources_mut(&mut self, visitor: &mut impl FnMut(&mut Embedded)) {
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
            fn inspect_resources(&self, visitor: &mut impl FnMut(&Embedded)) {
                for item in self.values() {
                    item.inspect_resources(visitor)
                }
            }

            fn inspect_resources_mut(&mut self, visitor: &mut impl FnMut(&mut Embedded)) {
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
            fn inspect_resources(&self, visitor: &mut impl FnMut(&Embedded)) {
                self.deref().inspect_resources(visitor)
            }

            fn inspect_resources_mut(&mut self, visitor: &mut impl FnMut(&mut Embedded)) {
                self.deref_mut().inspect_resources_mut(visitor)
            }
        }
    )*);
}

impl_deref_like!(Box);



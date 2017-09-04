
macro_rules! iter_impl{
    (_MK MUT $t:ty) => (
        &mut $t
    );
    (_MK REF $t:ty) => (
        & $t
    );
    (_MK2 MUT $lt:tt $t:ty ) => (
        & $lt mut $t
    );
    (_MK2 REF $lt:tt $t:ty) => (
        & $lt $t
    );
    (_MK EXPR MUT $e:expr) => (
        &mut $e
    );
    (_MK EXPR REF $e:expr) => (
        & $e
    );
    (
        $fn_name:ident, $tp_name:ident, $im:tt,
        map_scope: $mpn:ident
    ) => (
        use std;
        use $crate::codec::{ MailEncoder, MailEncodable };
        use $crate::headers::{ HeaderName, HeaderMap };

        impl<E: MailEncoder> HeaderMap<E> {
            pub fn $fn_name(self: iter_impl!{ _MK $im Self } ) -> $tp_name<E> {
                $tp_name {
                    header_map_iter: self.headers.$fn_name(),
                    sub_iter: None
                }
            }
        }
        type _AsIter<'a, E> =
            fn(iter_impl!{_MK2 $im 'a Vec<Box<MailEncodable<E>>> }) ->
                std::slice::$tp_name<'a, Box<MailEncodable<E>>>;

        type _Unbox<'a, E> =
            fn(iter_impl!{_MK2 $im 'a Box<MailEncodable<E>> }) ->
                iter_impl!{_MK2 $im 'a MailEncodable<E> };

        type _Iter<'a, E> = std::iter::Map<
            std::iter::FlatMap<
                std::option::$tp_name<
                    'a,
                    std::vec::Vec<Box<MailEncodable<E> + 'static>>
                >,
                std::slice::$tp_name<
                    'a,
                    Box<MailEncodable<E> + 'static>
                >,
                _AsIter<'a, E>
            >,
            _Unbox<'a, E>
        >;

        fn unbox<T: ?Sized>( v: iter_impl!{_MK $im Box<T>}) -> iter_impl!{_MK $im T} {
            iter_impl!{_MK EXPR $im **v }
        }

        pub struct $tp_name<'a, E: MailEncoder> {
            header_map_iter: $mpn::$tp_name<'a, HeaderName, HeaderBodies<E>>,
            sub_iter: Option<(HeaderName, _Iter<'a, E>)>
        }

        impl<'a, E> Iterator for $tp_name<'a, E>
            where E: MailEncoder
        {
            type Item = (HeaderName, iter_impl!{ _MK2 $im 'a MailEncodable<E> } );

            fn next(&mut self) -> Option<Self::Item> {
                let result = self.sub_iter.as_mut().and_then( |&mut (ref name, ref mut iter)| {
                    iter.next().map( |hbody| (*name, hbody) )
                } );
                if result.is_some() {
                    return result;
                } else {
                    let hbodies = self.header_map_iter.next();
                    if let Some( ( &name, hbodies) ) = hbodies {
                        let first = iter_impl!{_MK EXPR $im *hbodies.first };
                        self.sub_iter = Some( (
                            name,
                            hbodies.other
                                .$fn_name()
                                .flat_map::<_,fn(_)->_>( |e|e.$fn_name() )
                                .map( unbox::<MailEncodable<E>> as _Unbox<E> )
                        ) );
                        return Some( (name, first ) );
                    } else {
                        return None;
                    }
                }
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                let (min, _max) = self.header_map_iter.size_hint();
                // we have at last as many elements as the header map has entries,
                // but might have more
                (min, None)
            }
        }

    );
}
mod mut_iter {
    use std::collections::hash_map;
    use super::super::HeaderBodies;

    iter_impl!{
        iter_mut, IterMut, MUT,
        map_scope: hash_map
    }
}

mod ref_iter {
    use std::collections::hash_map;
    use super::super::HeaderBodies;

    iter_impl!{
        iter, Iter, REF,
        map_scope: hash_map
    }
}
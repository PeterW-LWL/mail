macro_rules! iter_impl {
    (_REF MUT $t:ty) => (
        &mut $t
    );
    (_REF REF $t:ty) => (
        & $t
    );
    (_REF2 MUT $lt:tt $t:ty ) => (
        & $lt mut $t
    );
    (_REF2 REF $lt:tt $t:ty) => (
        & $lt $t
    );
    (_REF EXPR MUT $e:expr) => (
        &mut $e
    );
    (_REF EXPR REF $e:expr) => (
        & $e
    );
    (
        $fn_name:ident, $tp_name:ident, $mutability:tt
    ) => (
        use std::slice;
        use $crate::codec::{ MailEncoder, MailEncodable };
        use $crate::headers::{ HeaderName, HeaderMap };

        impl<E: MailEncoder> HeaderMap<E> {
            pub fn $fn_name(self: iter_impl!{ _REF $mutability Self } ) -> $tp_name<E> {
                $tp_name {
                    vec_ptr_iter: self.header_vec.$fn_name()
                }
            }
        }

        pub struct $tp_name<'a, E: MailEncoder> {
            vec_ptr_iter: slice::$tp_name<'a, (HeaderName, *mut MailEncodable<E>)>
        }

        impl<'a, E> Iterator for $tp_name<'a, E>
            where E: MailEncoder
        {
            type Item = (HeaderName, iter_impl!{ _REF2 $mutability 'a MailEncodable<E> } );

            fn next(&mut self) -> Option<Self::Item> {
                self.vec_ptr_iter.next()
                    .map( |name_and_ptr| {
                        let name = name_and_ptr.0;
                        //SAFE: the signature of HeaderMap::iter/iter_mut gurantees
                        // that this is safe
                        let reference = unsafe { 
                            iter_impl!{ _REF EXPR $mutability *name_and_ptr.1  }
                        };
                        (name, reference)
                    })
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                self.vec_ptr_iter.size_hint()
            }
        }

    );
}

mod mut_iter {
    iter_impl!{
        iter_mut, IterMut, MUT
    }
}

mod ref_iter {
    iter_impl!{
        iter, Iter, REF
    }
}
use std::{mem, vec};

use codec::{ MailEncoder, MailEncodable };
use headers::HeaderName;

use super::HeaderMap;

impl<E> IntoIterator for HeaderMap<E>
    where E: MailEncoder
{
    type Item = (HeaderName, Box<MailEncodable<E>>);
    type IntoIter = IntoIter<E>;

    fn into_iter(self) -> Self::IntoIter {
        let HeaderMap { header_map, header_vec } = self;
        //TODO HeaderMap should use a ptr's for the header_map part
        //1. drop header_map without droping any of the data backing it
        for (_, bodies) in header_map.into_iter() {
            //we still have the handle in header_vec
            mem::forget(bodies.first);
            if let Some( other ) = bodies.other {
                for component_box in other.into_iter() {
                    mem::forget(component_box);
                }
            }
        }
        IntoIter {
            header_vec_iter: header_vec.into_iter()
        }
    }
}



pub struct IntoIter<E: MailEncoder> {
    header_vec_iter: vec::IntoIter<(HeaderName, *mut MailEncodable<E>)>
}

impl<E> Iterator for IntoIter<E>
    where E: MailEncoder
{
    type Item = (HeaderName, Box<MailEncodable<E>>);

    fn next(&mut self) -> Option<Self::Item> {
        self.header_vec_iter.next()
            .map(|(name, ptr)| {
                //SAFE: this *mut ptr is 1. unique and owining
                //  we can only get here by destructing a HeaderMap
                //  with into_iter, in which case we already removed
                //  the ptr's in header_map
                (name, unsafe { Box::from_raw(ptr) })
            })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.header_vec_iter.size_hint()
    }
}


#[cfg(test)]
mod test {
    use codec::{ MailEncodable, MailEncoderImpl};
    use headers::{ To, Subject, From};
    use components::Unstructured;
    use super::HeaderMap;
    use super::super::downcast_ref;

    #[test]
    fn into_iter() {
        const TEST_TEXT: &str = "this is a subject";
        let mut headers = HeaderMap::<MailEncoderImpl>::new();
        headers.insert(To, [ "affen@haus" ]).unwrap();
        headers.insert(Subject, TEST_TEXT).unwrap();
        headers.insert(From, [ "nix@da", "ja@wohl" ]).unwrap();

        let headers = headers.into_iter()
            .map(|(name, val)| {
                (name.as_str(), val)
            })
            .collect::<Vec<_>>();
        assert_eq!(3, headers.len());
        //check order
        assert_eq!("To", headers[0].0 );
        assert_eq!("Subject", headers[1].0);
        assert_eq!("From", headers[2].0 );

        //check if we can use the data
        let obj: &MailEncodable<_> = &*headers[1].1;
        let text = downcast_ref::<_, Unstructured>(obj).unwrap();
        assert_eq!(
            TEST_TEXT,
            text.as_str()
        );

    }
}
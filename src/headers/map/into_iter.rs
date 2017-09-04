use std;
use std::vec::{ IntoIter as VecIntoIter };
use std::iter::IntoIterator;

use codec::{ MailEncoder, MailEncodable };
use headers::HeaderName;


use super::MapIntoIter;
use super::HeaderBodies;
use super::HeaderMap;

impl<E> IntoIterator for HeaderMap<E>
    where E: MailEncoder
{
    type Item = (HeaderName, Box<MailEncodable<E>>);
    type IntoIter = IntoIter<E>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            header_map_iter: self.headers.into_iter(),
            sub_iter: None
        }
    }
}

type _Iter<E> = std::iter::FlatMap<
    std::option::IntoIter<
        std::vec::Vec<Box<MailEncodable<E> + 'static>>
    >,
    std::vec::IntoIter<
        Box<MailEncodable<E> + 'static>
    >,
    fn(Vec<Box<MailEncodable<E>>>) -> std::vec::IntoIter<Box<MailEncodable<E>>>
>;


pub struct IntoIter<E: MailEncoder> {
    header_map_iter: MapIntoIter<HeaderName, HeaderBodies<E>>,
    sub_iter: Option<(HeaderName, _Iter<E>)>
}

impl<E> Iterator for IntoIter<E>
    where E: MailEncoder
{
    type Item = (HeaderName, Box<MailEncodable<E>>);

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.sub_iter.as_mut().and_then( |&mut (ref name, ref mut iter)| {
            iter.next().map( |hbody| (*name, hbody) )
        } );
        if result.is_some() {
            return result;
        } else {
            let hbodies = self.header_map_iter.next();
            if let Some( ( name, HeaderBodies { first, other } ) ) = hbodies {
                self.sub_iter = Some( (
                    name,
                    other
                        .into_iter()
                        .flat_map( |el| el
                            .into_iter() )
                ) );
                return Some( (name, first) )
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

use std::collections;
use std::hash::Hash;
use std::mem::replace;

use serde;

use error::*;
use mail_composition::{
    DataInterface,
    EmbeddingInData,
    AttachmentInData
};



// impl D.I. for JSONLike


impl DataInterface for EmbeddingInData {

    fn find_externals<F1,F2>( &mut self, visit_emb: &mut F1, _vitis_att: &mut F2 ) -> Result<()>
        where F1: FnMut( &mut EmbeddingInData) -> Result<()>,
              F2: FnMut( &mut AttachmentInData) -> Result<()>
    {
        visit_emb( self )
    }

}

impl DataInterface for AttachmentInData {

    fn find_externals<F1,F2>( &mut self, _visit_emb: &mut F1, visit_att: &mut F2 ) -> Result<()>
        where F1: FnMut( &mut EmbeddingInData) -> Result<()>,
              F2: FnMut( &mut AttachmentInData) -> Result<()>
    {
        visit_att( self )
    }

}


impl<T: DataInterface> DataInterface for Vec<T> {

    fn find_externals<F1,F2>( &mut self, visit_emb: &mut F1, visit_att: &mut F2 ) -> Result<()>
        where F1: FnMut( &mut EmbeddingInData) -> Result<()>,
              F2: FnMut( &mut AttachmentInData) -> Result<()>
    {
        for ele in self.iter_mut() {
            ele.find_externals( visit_emb, visit_att )?;
        }
        Ok( () )
    }

}

impl<K, T: DataInterface> DataInterface for collections::HashMap<K, T>
    where K: Eq + Hash + serde::Serialize
{

    fn find_externals<F1,F2>( &mut self, visit_emb: &mut F1, visit_att: &mut F2 ) -> Result<()>
        where F1: FnMut( &mut EmbeddingInData) -> Result<()>,
              F2: FnMut( &mut AttachmentInData) -> Result<()>
    {
        for (_key, value) in self.iter_mut() {
            value.find_externals( visit_emb, visit_att )?;
        }
        Ok( () )
    }
}

impl<K, T: DataInterface> DataInterface for collections::BTreeMap<K, T>
    where K: Ord + serde::Serialize
{

    fn find_externals<F1,F2>( &mut self, visit_emb: &mut F1, visit_att: &mut F2 ) -> Result<()>
        where F1: FnMut( &mut EmbeddingInData) -> Result<()>,
              F2: FnMut( &mut AttachmentInData) -> Result<()>
    {
        for (_key, value) in self.iter_mut() {
            value.find_externals( visit_emb, visit_att )?;
        }
        Ok( () )
    }
}

impl<T: DataInterface + Eq + Hash> DataInterface for collections::HashSet<T> {

    fn find_externals<F1,F2>( &mut self, visit_emb: &mut F1, visit_att: &mut F2 ) -> Result<()>
        where F1: FnMut( &mut EmbeddingInData) -> Result<()>,
              F2: FnMut( &mut AttachmentInData) -> Result<()>
    {
        let capacity = self.capacity();
        let old = replace( self, Self::with_capacity( capacity ) );
        let mut iter = old.into_iter();
        while let Some( mut el ) = iter.next() {
            if let Err( err ) = el.find_externals( visit_emb, visit_att ) {
                self.extend( iter );
                return Err( err )
            }
            self.insert( el );
        }
        Ok( () )
    }
}


impl<T: DataInterface + Ord> DataInterface for collections::BTreeSet<T> {

    fn find_externals<F1,F2>( &mut self, visit_emb: &mut F1, visit_att: &mut F2 ) -> Result<()>
        where F1: FnMut( &mut EmbeddingInData) -> Result<()>,
              F2: FnMut( &mut AttachmentInData) -> Result<()>
    {
        let old = replace( self, Self::new() );
        let mut iter = old.into_iter();
        while let Some( mut el ) = iter.next() {
            if let Err( err ) = el.find_externals( visit_emb, visit_att ) {
                self.extend( iter );
                return Err( err )
            }
            self.insert( el );
        }
        Ok( () )
    }
}

impl<T: DataInterface> DataInterface for collections::VecDeque<T> {

    fn find_externals<F1,F2>( &mut self, visit_emb: &mut F1, visit_att: &mut F2 ) -> Result<()>
        where F1: FnMut( &mut EmbeddingInData) -> Result<()>,
              F2: FnMut( &mut AttachmentInData) -> Result<()>
    {
        for mut value in self.iter_mut() {
            value.find_externals( visit_emb, visit_att )?;
        }
        Ok( () )
    }
}


impl<T: DataInterface> DataInterface for collections::LinkedList<T> {

    fn find_externals<F1,F2>( &mut self, visit_emb: &mut F1, visit_att: &mut F2 ) -> Result<()>
        where F1: FnMut( &mut EmbeddingInData) -> Result<()>,
              F2: FnMut( &mut AttachmentInData) -> Result<()>
    {
        for ele in self.iter_mut() {
            ele.find_externals( visit_emb, visit_att )?;
        }
        Ok( () )
    }

}


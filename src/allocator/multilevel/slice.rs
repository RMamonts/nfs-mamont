use std::mem;

use crate::allocator::slice::{Iter, IterMut};
use crate::allocator::{Sender, Slice};

pub enum MultiSlice {
    One(Slice),
    Cons(Slice, Box<MultiSlice>),
}

impl MultiSlice {
    pub fn new(
        buffers: Vec<Box<[u8]>>,
        range: std::ops::Range<usize>,
        sender: Sender<Box<[u8]>>,
        next: Option<Box<MultiSlice>>,
    ) -> Self {
        let slice = Slice::new(buffers, range, sender);
        match next {
            None => MultiSlice::One(slice),
            Some(next) => MultiSlice::Cons(slice, next),
        }
    }

    pub fn total_len(&self) -> usize {
        match self {
            MultiSlice::One(s) => s.buffers.iter().map(|b| b.len()).sum(),
            MultiSlice::Cons(s, rest) => {
                s.buffers.iter().map(|b| b.len()).sum::<usize>() + rest.total_len()
            }
        }
    }

    pub fn empty() -> Self {
        MultiSlice::One(Slice::empty())
    }

    pub fn iter(&self) -> MultiIter<'_> {
        self.into_iter()
    }
    pub fn iter_mut(&mut self) -> MultiMutIter<'_> {
        self.into_iter()
    }

    fn deallocate(&mut self) {
        match self {
            MultiSlice::One(slice) => slice.deallocate(),
            MultiSlice::Cons(slice, next) => {
                slice.deallocate();
                next.deallocate();
            }
        }
    }
}

impl Drop for MultiSlice {
    fn drop(&mut self) {
        self.deallocate();
    }
}

pub struct MultiIter<'a> {
    current_iter: Iter<'a>,
    next_iter: Option<Box<MultiIter<'a>>>,
}

impl<'a> Iterator for MultiIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.current_iter.next() {
            return Some(next);
        }
        if let Some(next) = self.next_iter.take() {
            let _ = mem::replace(&mut self.current_iter, next.current_iter);
            let _ = mem::replace(&mut self.next_iter, next.next_iter);
            self.next()
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for &'a MultiSlice {
    type Item = &'a [u8];
    type IntoIter = MultiIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        match self {
            MultiSlice::One(s) => MultiIter { current_iter: s.into_iter(), next_iter: None },
            MultiSlice::Cons(s, rest) => MultiIter {
                current_iter: s.into_iter(),
                next_iter: Some(Box::new(Self::into_iter(rest))),
            },
        }
    }
}

pub struct MultiMutIter<'a> {
    current_iter: IterMut<'a>,
    next_iter: Option<Box<MultiMutIter<'a>>>,
}

impl<'a> Iterator for MultiMutIter<'a> {
    type Item = &'a mut [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.current_iter.next() {
            return Some(next);
        }
        if let Some(next) = self.next_iter.take() {
            let _ = mem::replace(&mut self.current_iter, next.current_iter);
            let _ = mem::replace(&mut self.next_iter, next.next_iter);
            self.next()
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for &'a mut MultiSlice {
    type Item = &'a mut [u8];
    type IntoIter = MultiMutIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        match self {
            MultiSlice::One(s) => MultiMutIter { current_iter: s.into_iter(), next_iter: None },
            MultiSlice::Cons(s, rest) => MultiMutIter {
                current_iter: s.into_iter(),
                next_iter: Some(Box::new(Self::into_iter(rest))),
            },
        }
    }
}

use super::List;

pub struct Slice {
    list: List,
    range: std::ops::Range<usize>,
}

impl Slice {
    pub fn new(list: super::list::List, range: std::ops::Range<usize>) -> Self {
        assert!(range.start <= list.len(), "cannot index list as slice from start");
        assert!(range.end <= list.len(), "cannot index list as slice to end");

        Self { list, range }
    }
}

pub struct IterMut<'a> {
    slice_iter: crate::allocator::list::IterMut<'a>,
    range: std::ops::Range<usize>,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut [u8];
    fn next(&mut self) -> Option<Self::Item> {
        let result = &mut self.slice_iter.next()?[self.range.clone()];

        self.range.start = self.range.start.saturating_sub(result.len());
        self.range.end = self.range.end.saturating_sub(result.len());

        Some(result)
    }
}

impl<'a> IntoIterator for &'a mut Slice {
    type IntoIter = IterMut<'a>;
    type Item = &'a mut [u8];

    fn into_iter(self) -> Self::IntoIter {
        IterMut { slice_iter: (&mut self.list).into_iter(), range: self.range.clone() }
    }
}

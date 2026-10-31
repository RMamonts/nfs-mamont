//! Small-vector of runs: the chunk list of a buffer without a heap allocation in the
//! common case.

/// `len` consecutive units starting at unit `start`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Run {
    pub start: u32,
    pub len: u32,
}

/// Inline capacity. 16 runs cover a 64 KiB request served by 4 KiB fixed blocks.
pub const INLINE: usize = 16;

pub enum Runs {
    Inline { len: u8, items: [Run; INLINE] },
    Heap(Vec<Run>),
}

impl Default for Runs {
    fn default() -> Self {
        Self::new()
    }
}

impl Runs {
    #[inline]
    pub const fn new() -> Self {
        Runs::Inline { len: 0, items: [Run { start: 0, len: 0 }; INLINE] }
    }

    #[inline]
    pub fn push(&mut self, run: Run) {
        match self {
            Runs::Inline { len, items } => {
                let n = *len as usize;
                if n < INLINE {
                    items[n] = run;
                    *len += 1;
                } else {
                    let mut v = Vec::with_capacity(INLINE * 2);
                    v.extend_from_slice(items);
                    v.push(run);
                    *self = Runs::Heap(v);
                }
            }
            Runs::Heap(v) => v.push(run),
        }
    }

    /// Appends `run`, merging it into the last run when they are adjacent.
    #[inline]
    pub fn push_coalesce(&mut self, run: Run) {
        if let Some(last) = self.last_mut() {
            if last.start + last.len == run.start {
                last.len += run.len;
                return;
            }
        }
        self.push(run);
    }

    #[inline]
    fn last_mut(&mut self) -> Option<&mut Run> {
        match self {
            Runs::Inline { len, items } => {
                if *len == 0 {
                    None
                } else {
                    Some(&mut items[*len as usize - 1])
                }
            }
            Runs::Heap(v) => v.last_mut(),
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[Run] {
        match self {
            Runs::Inline { len, items } => &items[..*len as usize],
            Runs::Heap(v) => v,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Total number of units covered by the runs.
    pub fn units(&self) -> usize {
        self.as_slice().iter().map(|r| r.len as usize).sum()
    }
}

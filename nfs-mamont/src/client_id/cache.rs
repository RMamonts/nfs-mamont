use std::collections::VecDeque;

const CAPACITY: usize = 1024;

/// Bounded FIFO cache. Insertion and eviction are performed only by `add`.
pub struct Cache<T> {
    entries: VecDeque<T>,
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self { entries: VecDeque::new() }
    }
}

impl<T> Cache<T> {
    pub fn add(&mut self, entry: T) {
        if self.entries.len() == CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn find(&self, mut predicate: impl FnMut(&T) -> bool) -> Option<&T> {
        self.entries.iter().find(|entry| predicate(entry))
    }

    pub fn retain(&mut self, predicate: impl FnMut(&T) -> bool) {
        self.entries.retain(predicate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_evicts_oldest_and_lookup_does_not_refresh_it() {
        let mut cache = Cache::default();
        for value in 0..CAPACITY {
            cache.add(value);
        }
        assert_eq!(cache.find(|value| *value == 0), Some(&0));
        cache.add(CAPACITY);
        assert_eq!(cache.find(|value| *value == 0), None);
        assert_eq!(cache.find(|value| *value == 1), Some(&1));
        assert_eq!(cache.find(|value| *value == CAPACITY), Some(&CAPACITY));
        assert_eq!(cache.entries.len(), CAPACITY);
    }
}

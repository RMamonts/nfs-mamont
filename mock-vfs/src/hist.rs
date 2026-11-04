//! Log-linear histogram for latency samples.
//!
//! Values (microseconds in the tools) below 32 are counted exactly; above that every
//! power of two is split into 32 sub-buckets, which bounds the relative error of a
//! percentile to about 3 % while keeping the histogram at a few kilobytes.

/// Number of bits of mantissa kept per octave.
const SUB_BUCKET_BITS: usize = 5;
/// Sub-buckets per octave.
const SUB_BUCKETS: usize = 1 << SUB_BUCKET_BITS;
/// Total number of buckets covering the whole `u64` range.
const BUCKETS: usize = (64 - SUB_BUCKET_BITS + 1) * SUB_BUCKETS;

/// Histogram of `u64` samples.
#[derive(Clone, Debug)]
pub struct Histogram {
    buckets: Vec<u64>,
    count: u64,
    sum: u64,
    min: u64,
    max: u64,
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

impl Histogram {
    /// Creates an empty histogram.
    pub fn new() -> Self {
        Self { buckets: vec![0; BUCKETS], count: 0, sum: 0, min: u64::MAX, max: 0 }
    }

    /// Adds one sample.
    pub fn record(&mut self, value: u64) {
        self.buckets[bucket(value)] += 1;
        self.count += 1;
        self.sum = self.sum.saturating_add(value);
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }

    /// Adds every sample of `other`.
    pub fn merge(&mut self, other: &Histogram) {
        for (mine, theirs) in self.buckets.iter_mut().zip(&other.buckets) {
            *mine += theirs;
        }
        self.count += other.count;
        self.sum = self.sum.saturating_add(other.sum);
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    /// Returns the number of samples.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Returns the smallest sample.
    pub fn min(&self) -> Option<u64> {
        (self.count > 0).then_some(self.min)
    }

    /// Returns the largest sample.
    pub fn max(&self) -> Option<u64> {
        (self.count > 0).then_some(self.max)
    }

    /// Returns the arithmetic mean of the samples.
    pub fn mean(&self) -> Option<f64> {
        (self.count > 0).then(|| self.sum as f64 / self.count as f64)
    }

    /// Returns the value below which `percentile` percent of the samples fall, with
    /// the resolution of the histogram.
    pub fn percentile(&self, percentile: f64) -> Option<u64> {
        if self.count == 0 {
            return None;
        }
        let target = ((percentile / 100.0) * self.count as f64).ceil() as u64;
        let target = target.clamp(1, self.count);
        if target == self.count {
            return Some(self.max);
        }

        let mut seen = 0;
        for (index, &samples) in self.buckets.iter().enumerate() {
            seen += samples;
            if seen >= target {
                return Some(lower_bound(index).clamp(self.min, self.max));
            }
        }

        Some(self.max)
    }
}

/// Returns the bucket `value` falls into.
fn bucket(value: u64) -> usize {
    if value < SUB_BUCKETS as u64 {
        return value as usize;
    }
    let exponent = 63 - value.leading_zeros() as usize;
    let mantissa = ((value >> (exponent - SUB_BUCKET_BITS)) & (SUB_BUCKETS as u64 - 1)) as usize;

    (exponent - SUB_BUCKET_BITS + 1) * SUB_BUCKETS + mantissa
}

/// Returns the smallest value of bucket `index`.
fn lower_bound(index: usize) -> u64 {
    if index < SUB_BUCKETS {
        return index as u64;
    }
    let exponent = index / SUB_BUCKETS - 1 + SUB_BUCKET_BITS;
    let mantissa = (index % SUB_BUCKETS) as u64;

    (1u64 << exponent) + (mantissa << (exponent - SUB_BUCKET_BITS))
}

#[cfg(test)]
mod tests {
    use super::{bucket, lower_bound, Histogram, BUCKETS};

    #[test]
    fn buckets_are_contiguous_and_cover_the_range() {
        let mut previous = bucket(0);
        for value in 1..100_000u64 {
            let current = bucket(value);
            assert!(current == previous || current == previous + 1, "gap at {value}");
            assert!(lower_bound(current) <= value, "lower bound above {value}");
            previous = current;
        }
        assert!(bucket(u64::MAX) < BUCKETS);
        assert_eq!(lower_bound(bucket(64)), 64);
        assert_eq!(lower_bound(bucket(1000)), 992);
    }

    #[test]
    fn percentiles_are_within_the_resolution() {
        let mut hist = Histogram::new();
        for value in 1..=10_000u64 {
            hist.record(value);
        }

        assert_eq!(hist.count(), 10_000);
        assert_eq!(hist.min(), Some(1));
        assert_eq!(hist.max(), Some(10_000));
        assert_eq!(hist.mean(), Some(5000.5));

        let p50 = hist.percentile(50.0).unwrap();
        assert!((4800..=5000).contains(&p50), "p50 = {p50}");
        let p99 = hist.percentile(99.0).unwrap();
        assert!((9600..=9900).contains(&p99), "p99 = {p99}");
        assert_eq!(hist.percentile(100.0), Some(10_000));
        assert_eq!(hist.percentile(0.0), Some(1));
    }

    #[test]
    fn merge_combines_samples() {
        let mut a = Histogram::new();
        let mut b = Histogram::new();
        a.record(10);
        b.record(30);
        b.record(50);

        a.merge(&b);

        assert_eq!(a.count(), 3);
        assert_eq!(a.min(), Some(10));
        assert_eq!(a.max(), Some(50));
        assert_eq!(a.mean(), Some(30.0));
    }

    #[test]
    fn empty_histogram_has_no_statistics() {
        let hist = Histogram::new();

        assert_eq!(hist.percentile(50.0), None);
        assert_eq!(hist.mean(), None);
        assert_eq!(hist.min(), None);
    }
}

use crate::service::nlm::{calculate_end_of_interval, calculate_len_of_interval, ranges_overlap};

#[test]
fn overlapping_ranges_detect_overlap() {
    assert!(ranges_overlap(0, 10, 5, 10));
}

#[test]
fn non_overlapping_ranges_no_overlap() {
    assert!(!ranges_overlap(0, 10, 10, 10));
}

#[test]
fn identical_ranges_overlap() {
    assert!(ranges_overlap(42, 100, 42, 100));
}

#[test]
fn inner_range_contained_overlaps() {
    assert!(ranges_overlap(0, 100, 25, 50));
}

#[test]
fn zero_length_ranges_overlap() {
    assert!(ranges_overlap(0, 0, 0, 0));
}

#[test]
fn zero_length_means_to_eof() {
    assert!(ranges_overlap(0, 0, 100, 50));
}

#[test]
fn zero_length_does_not_overlap_before() {
    assert!(!ranges_overlap(100, 0, 0, 50));
}

#[test]
fn range_at_the_last_byte_overlaps_itself() {
    // `[u64::MAX, u64::MAX]` used to end at `u64::MAX - 1`, i.e. before it
    // started, so it overlapped nothing --- two clients could hold an exclusive
    // lock on that byte at once.
    assert!(ranges_overlap(u64::MAX, 1, u64::MAX, 1));
    assert!(ranges_overlap(u64::MAX, 1, u64::MAX - 1, 2));
}

#[test]
fn range_end_is_clamped_at_the_last_byte() {
    assert_eq!(calculate_end_of_interval(u64::MAX, 1), u64::MAX);
    assert_eq!(calculate_end_of_interval(u64::MAX - 1, 4), u64::MAX);
    assert_eq!(calculate_end_of_interval(0, 10), 9);
    assert_eq!(calculate_end_of_interval(0, 0), u64::MAX);
}

#[test]
fn interval_length_inverts_interval_end() {
    assert_eq!(calculate_len_of_interval(0, 9), 10);
    assert_eq!(calculate_len_of_interval(5, 5), 1);
    // A range covering the whole address space has no representable length and
    // is stored as "to end-of-file".
    assert_eq!(calculate_len_of_interval(0, u64::MAX), 0);
}

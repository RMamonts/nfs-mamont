use super::super::ranges_overlap;

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

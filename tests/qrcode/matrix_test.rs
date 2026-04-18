use viv::qrcode::matrix::QrMatrix;

#[test]
fn matrix_size_v1() {
    let m = QrMatrix::new(1);
    assert_eq!(m.size(), 21);
}

#[test]
fn matrix_size_v5() {
    let m = QrMatrix::new(5);
    assert_eq!(m.size(), 37);
}

#[test]
fn matrix_size_v40() {
    let m = QrMatrix::new(40);
    assert_eq!(m.size(), 177);
}

#[test]
fn finder_pattern_top_left() {
    let m = QrMatrix::new(1);
    // Corners of the finder are black
    assert!(m.get(0, 0));
    assert!(m.get(0, 6));
    assert!(m.get(6, 0));
    assert!(m.get(6, 6));
    // Inside ring is white
    assert!(!m.get(1, 1));
    assert!(!m.get(1, 5));
    // Center is black
    assert!(m.get(2, 2));
    assert!(m.get(3, 3));
    assert!(m.get(4, 4));
}

#[test]
fn finder_pattern_top_right() {
    let m = QrMatrix::new(1);
    let s = m.size();
    assert!(m.get(0, s - 1));
    assert!(m.get(0, s - 7));
    assert!(m.get(3, s - 4)); // center
}

#[test]
fn finder_pattern_bottom_left() {
    let m = QrMatrix::new(1);
    let s = m.size();
    assert!(m.get(s - 1, 0));
    assert!(m.get(s - 7, 0));
    assert!(m.get(s - 4, 3)); // center
}

#[test]
fn separator_white() {
    let m = QrMatrix::new(1);
    // Row 7 between top-left finder and rest should be white
    assert!(!m.get(7, 0));
    assert!(!m.get(7, 6));
    assert!(!m.get(7, 7));
}

#[test]
fn timing_pattern() {
    let m = QrMatrix::new(1);
    // Row 6, col 8+ alternates: 8=black, 9=white, 10=black...
    assert!(m.get(6, 8));
    assert!(!m.get(6, 9));
    assert!(m.get(6, 10));
    assert!(!m.get(6, 11));
    assert!(m.get(6, 12));
}

#[test]
fn dark_module_v1() {
    let m = QrMatrix::new(1);
    // Dark module at (4*1+9, 8) = (13, 8)
    assert!(m.get(13, 8));
}

#[test]
fn build_v1_correct_size() {
    let m = QrMatrix::build(1, &vec![0u8; 26]);
    assert_eq!(m.size(), 21);
}

#[test]
fn alignment_pattern_v2() {
    let m = QrMatrix::new(2);
    // V2 alignment at (6, 18) and (18, 6) and (18, 18)
    // But (6, 6) overlaps finder -> skipped
    // (18, 18) center should be black
    assert!(m.get(18, 18));
    // Ring around center should be white
    assert!(!m.get(17, 18));
    assert!(!m.get(19, 18));
}

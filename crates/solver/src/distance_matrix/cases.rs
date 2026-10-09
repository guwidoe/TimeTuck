// Adapted from Timefold Community LongArrayDistanceMatrixTest#getIndex.
// SPDX-License-Identifier: Apache-2.0
use super::pair_index;
#[test]
fn get_index() {
    let expected: &[&[usize]] = &[
        &[0, 2, 5, 9, 14, 20, 27, 35],
        &[1, 4, 8, 13, 19, 26, 34],
        &[3, 7, 12, 18, 25, 33],
        &[6, 11, 17, 24, 32],
        &[10, 16, 23, 31],
        &[15, 22, 30],
        &[21, 29],
        &[28],
    ];
    for (from, row) in expected.iter().enumerate() {
        for to in from..row.len() {
            assert_eq!(pair_index(from, to), row[to - from], "{from},{to}");
        }
    }
}

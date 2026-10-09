// Adapted from pinned Timefold Community StaticDaypartBucketingTest.
// SPDX-License-Identifier: Apache-2.0
use timetuck::{
    Daypart::{self, *},
    OffsetDeparture,
};
fn at(hour: i64, minute: i64) -> OffsetDeparture {
    OffsetDeparture::new(1_704_067_200 + hour * 3600 + minute * 60, 0).unwrap()
}
#[test]
fn boundaries_are_half_open() {
    for (h, m) in [(0, 0), (5, 59), (18, 0), (23, 59)] {
        assert_eq!(Daypart::at(at(h, m)), Night);
    }
    for (h, m) in [(6, 0), (11, 59)] {
        assert_eq!(Daypart::at(at(h, m)), Morning);
    }
    for (h, m) in [(12, 0), (17, 59)] {
        assert_eq!(Daypart::at(at(h, m)), Afternoon);
    }
}
#[test]
fn buckets_use_inputs_own_offset() {
    let utc = at(10, 0);
    let plus2 = utc.with_offset(7200).unwrap();
    assert_eq!(utc.unix_seconds(), plus2.unix_seconds());
    assert_eq!(plus2.local_hour(), 12);
    assert_eq!(Daypart::at(utc), Morning);
    assert_eq!(Daypart::at(plus2), Afternoon);
}
#[test]
fn all_timeframes_order() {
    assert_eq!(Daypart::ALL, [Morning, Afternoon, Night]);
}
#[test]
fn index_matches_order() {
    assert_eq!(Daypart::at(at(8, 0)).index(), 0);
    assert_eq!(Daypart::at(at(14, 0)).index(), 1);
    assert_eq!(Daypart::at(at(22, 0)).index(), 2);
    for h in 0..24 {
        let p = Daypart::at(at(h, 0));
        assert_eq!(Daypart::ALL[p.index()], p);
    }
}
#[test]
fn narrow_interval() {
    assert_eq!(
        Daypart::covering(at(9, 0), at(11, 0)).unwrap(),
        vec![Morning]
    );
}
#[test]
fn two_buckets() {
    assert_eq!(
        Daypart::covering(at(9, 0), at(14, 0)).unwrap(),
        vec![Morning, Afternoon]
    );
}
#[test]
fn end_boundary_is_exclusive() {
    assert_eq!(
        Daypart::covering(at(9, 0), at(18, 0)).unwrap(),
        vec![Morning, Afternoon]
    );
}
#[test]
fn just_past_boundary() {
    assert_eq!(
        Daypart::covering(at(9, 0), at(18, 1)).unwrap(),
        Daypart::ALL
    );
}
#[test]
fn zero_length_covers_start() {
    assert_eq!(
        Daypart::covering(at(12, 0), at(12, 0)).unwrap(),
        vec![Afternoon]
    );
}

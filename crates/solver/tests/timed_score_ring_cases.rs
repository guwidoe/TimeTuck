// Adapted from pinned Community DiminishedReturnsScoreRingBufferTest.
// SPDX-License-Identifier: Apache-2.0
// Exact physical storage/cursors retained; native timestamps use u128 and
// explicit active length distinguishes full/empty without a zero sentinel.
use timetuck::*;

#[test]
fn test_resize_basic() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 1,
        write: 3,
        len: 2,
        times: vec![0, 2, 3, 0],
        scores: vec![
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
        ],
    })
    .unwrap();
    buffer.grow().unwrap();
    assert_eq!(buffer.state().read, 0);
    assert_eq!(buffer.state().write, 2);
    assert_eq!(buffer.state().times, vec![2, 3, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            None,
            None,
            None,
            None,
            None,
            None
        ]
    );
}

#[test]
fn test_resize_wrapped() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 3,
        write: 1,
        len: 2,
        times: vec![3, 0, 0, 2],
        scores: vec![
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
        ],
    })
    .unwrap();
    buffer.grow().unwrap();
    assert_eq!(buffer.state().read, 0);
    assert_eq!(buffer.state().write, 2);
    assert_eq!(buffer.state().times, vec![2, 3, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            None,
            None,
            None,
            None,
            None,
            None
        ]
    );
}

#[test]
fn test_put_empty() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 0,
        write: 0,
        len: 0,
        times: vec![0, 0, 0, 0],
        scores: vec![None, None, None, None],
    })
    .unwrap();
    buffer.push(1, SimpleScore::new(1)).unwrap();
    assert_eq!(buffer.state().read, 0);
    assert_eq!(buffer.state().write, 1);
    assert_eq!(buffer.state().times, vec![1, 0, 0, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![Some(SimpleScore::new(1)), None, None, None]
    );
    buffer.push(3, SimpleScore::new(2)).unwrap();
    assert_eq!(buffer.state().read, 0);
    assert_eq!(buffer.state().write, 2);
    assert_eq!(buffer.state().times, vec![1, 3, 0, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
            None,
            None
        ]
    );
    buffer.push(5, SimpleScore::new(3)).unwrap();
    assert_eq!(buffer.state().read, 0);
    assert_eq!(buffer.state().write, 3);
    assert_eq!(buffer.state().times, vec![1, 3, 5, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            None
        ]
    );
    buffer.push(7, SimpleScore::new(4)).unwrap();
    assert_eq!(buffer.state().read, 0);
    assert_eq!(buffer.state().write, 0);
    assert_eq!(buffer.state().times, vec![1, 3, 5, 7]);
    assert_eq!(
        buffer.state().scores,
        vec![
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4))
        ]
    );
    buffer.push(9, SimpleScore::new(5)).unwrap();
    assert_eq!(buffer.state().read, 0);
    assert_eq!(buffer.state().write, 5);
    assert_eq!(buffer.state().times, vec![1, 3, 5, 7, 9, 0, 0, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            Some(SimpleScore::new(5)),
            None,
            None,
            None
        ]
    );
}

#[test]
fn test_put_wrapped() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 2,
        write: 0,
        len: 2,
        times: vec![0, 0, 1, 3],
        scores: vec![
            Some(SimpleScore::new(-1)),
            Some(SimpleScore::new(0)),
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
        ],
    })
    .unwrap();
    buffer.push(5, SimpleScore::new(3)).unwrap();
    assert_eq!(buffer.state().read, 2);
    assert_eq!(buffer.state().write, 1);
    assert_eq!(buffer.state().times, vec![5, 0, 1, 3]);
    assert_eq!(
        buffer.state().scores,
        vec![
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(0)),
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2))
        ]
    );
    buffer.push(7, SimpleScore::new(4)).unwrap();
    assert_eq!(buffer.state().read, 2);
    assert_eq!(buffer.state().write, 2);
    assert_eq!(buffer.state().times, vec![5, 7, 1, 3]);
    assert_eq!(
        buffer.state().scores,
        vec![
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2))
        ]
    );
    buffer.push(9, SimpleScore::new(5)).unwrap();
    assert_eq!(buffer.state().read, 0);
    assert_eq!(buffer.state().write, 5);
    assert_eq!(buffer.state().times, vec![1, 3, 5, 7, 9, 0, 0, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            Some(SimpleScore::new(5)),
            None,
            None,
            None
        ]
    );
}

#[test]
fn test_peek() {
    let buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 2,
        write: 0,
        len: 2,
        times: vec![0, 0, 1, 3],
        scores: vec![
            Some(SimpleScore::new(-1)),
            Some(SimpleScore::new(0)),
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
        ],
    })
    .unwrap();
    assert_eq!(buffer.peek().unwrap(), &SimpleScore::new(1));
    assert_eq!(buffer.state().read, 2);
    assert_eq!(buffer.state().write, 0);
    assert_eq!(buffer.state().times, vec![0, 0, 1, 3]);
    assert_eq!(
        buffer.state().scores,
        vec![
            Some(SimpleScore::new(-1)),
            Some(SimpleScore::new(0)),
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2))
        ]
    );
}

#[test]
fn test_peek_empty() {
    let buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 0,
        write: 0,
        len: 0,
        times: vec![0, 0, 0, 0],
        scores: vec![None, None, None, None],
    })
    .unwrap();
    assert_eq!(buffer.peek(), Err(TimedScoreRingError::Empty));
    assert!(
        buffer
            .peek()
            .unwrap_err()
            .to_string()
            .contains("buffer is empty")
    );
}

#[test]
fn test_clear() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 2,
        write: 0,
        len: 2,
        times: vec![0, 0, 1, 3],
        scores: vec![
            None,
            None,
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
        ],
    })
    .unwrap();
    buffer.clear();
    assert_eq!(buffer.state().read, 0);
    assert_eq!(buffer.state().write, 0);
    assert_eq!(buffer.state().times, vec![0, 0, 0, 0]);
    assert_eq!(buffer.state().scores, vec![None, None, None, None]);
}

#[test]
fn test_poll_latest_score_before_time_and_clear_prior_full() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 0,
        write: 0,
        len: 4,
        times: vec![1, 2, 3, 4],
        scores: vec![
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
        ],
    })
    .unwrap();
    assert_eq!(buffer.retain_floor(2).unwrap().1, &SimpleScore::new(2));
    assert_eq!(buffer.state().read, 1);
    assert_eq!(buffer.state().write, 0);
    assert_eq!(buffer.state().times, vec![0, 2, 3, 4]);
    assert_eq!(
        buffer.state().scores,
        vec![
            None,
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4))
        ]
    );
}

#[test]
fn test_poll_latest_score_before_time_and_clear_prior_empty() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 0,
        write: 0,
        len: 0,
        times: vec![0, 0, 0, 0],
        scores: vec![None, None, None, None],
    })
    .unwrap();
    assert_eq!(buffer.retain_floor(10), Err(TimedScoreRingError::Empty));
}

#[test]
fn test_poll_latest_score_before_time_and_clear_prior_equal_normal() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 0,
        write: 3,
        len: 3,
        times: vec![1, 2, 3, 0],
        scores: vec![
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            None,
        ],
    })
    .unwrap();
    assert_eq!(buffer.retain_floor(2).unwrap().1, &SimpleScore::new(2));
    assert_eq!(buffer.state().read, 1);
    assert_eq!(buffer.state().write, 3);
    assert_eq!(buffer.state().times, vec![0, 2, 3, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![
            None,
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            None
        ]
    );
}

#[test]
fn test_poll_latest_score_before_time_and_clear_prior_less_than_normal() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 0,
        write: 3,
        len: 3,
        times: vec![1, 2, 4, 0],
        scores: vec![
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            None,
        ],
    })
    .unwrap();
    assert_eq!(buffer.retain_floor(3).unwrap().1, &SimpleScore::new(2));
    assert_eq!(buffer.state().read, 1);
    assert_eq!(buffer.state().write, 3);
    assert_eq!(buffer.state().times, vec![0, 2, 4, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![
            None,
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            None
        ]
    );
}

#[test]
fn test_poll_latest_score_before_time_and_clear_prior_overflow_normal() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 0,
        write: 3,
        len: 3,
        times: vec![1, 2, 3, 0],
        scores: vec![
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            None,
        ],
    })
    .unwrap();
    assert_eq!(buffer.retain_floor(10).unwrap().1, &SimpleScore::new(3));
    assert_eq!(buffer.state().read, 2);
    assert_eq!(buffer.state().write, 3);
    assert_eq!(buffer.state().times, vec![0, 0, 3, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![None, None, Some(SimpleScore::new(3)), None]
    );
}

#[test]
fn test_poll_latest_score_before_time_and_clear_prior_equal_wrapped() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 4,
        write: 3,
        len: 4,
        times: vec![2, 3, 4, 0, 1],
        scores: vec![
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            None,
            Some(SimpleScore::new(1)),
        ],
    })
    .unwrap();
    assert_eq!(buffer.retain_floor(3).unwrap().1, &SimpleScore::new(3));
    assert_eq!(buffer.state().read, 1);
    assert_eq!(buffer.state().write, 3);
    assert_eq!(buffer.state().times, vec![0, 3, 4, 0, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![
            None,
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            None,
            None
        ]
    );
}

#[test]
fn test_poll_latest_score_before_time_and_clear_prior_less_than_wrapped() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 4,
        write: 3,
        len: 4,
        times: vec![2, 3, 5, 0, 1],
        scores: vec![
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            None,
            Some(SimpleScore::new(1)),
        ],
    })
    .unwrap();
    assert_eq!(buffer.retain_floor(4).unwrap().1, &SimpleScore::new(3));
    assert_eq!(buffer.state().read, 1);
    assert_eq!(buffer.state().write, 3);
    assert_eq!(buffer.state().times, vec![0, 3, 5, 0, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![
            None,
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            None,
            None
        ]
    );
}

#[test]
fn test_poll_latest_score_before_time_and_clear_prior_overflow_wrapped() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 4,
        write: 3,
        len: 4,
        times: vec![2, 3, 4, 0, 1],
        scores: vec![
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            None,
            Some(SimpleScore::new(1)),
        ],
    })
    .unwrap();
    assert_eq!(buffer.retain_floor(10).unwrap().1, &SimpleScore::new(4));
    assert_eq!(buffer.state().read, 2);
    assert_eq!(buffer.state().write, 3);
    assert_eq!(buffer.state().times, vec![0, 0, 4, 0, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![None, None, Some(SimpleScore::new(4)), None, None]
    );
}

#[test]
fn test_poll_latest_score_before_time_and_clear_prior_equal_wrapped_zero() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 4,
        write: 3,
        len: 4,
        times: vec![2, 3, 4, 0, 1],
        scores: vec![
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            None,
            Some(SimpleScore::new(1)),
        ],
    })
    .unwrap();
    assert_eq!(buffer.retain_floor(2).unwrap().1, &SimpleScore::new(2));
    assert_eq!(buffer.state().read, 0);
    assert_eq!(buffer.state().write, 3);
    assert_eq!(buffer.state().times, vec![2, 3, 4, 0, 0]);
    assert_eq!(
        buffer.state().scores,
        vec![
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            None,
            None
        ]
    );
}

#[test]
fn test_poll_latest_score_before_time_and_clear_prior_less_than_wrapped_zero() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 4,
        write: 3,
        len: 4,
        times: vec![3, 4, 5, 0, 1],
        scores: vec![
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            None,
            Some(SimpleScore::new(1)),
        ],
    })
    .unwrap();
    assert_eq!(buffer.retain_floor(2).unwrap().1, &SimpleScore::new(1));
    assert_eq!(buffer.state().read, 4);
    assert_eq!(buffer.state().write, 3);
    assert_eq!(buffer.state().times, vec![3, 4, 5, 0, 1]);
    assert_eq!(
        buffer.state().scores,
        vec![
            Some(SimpleScore::new(2)),
            Some(SimpleScore::new(3)),
            Some(SimpleScore::new(4)),
            None,
            Some(SimpleScore::new(1))
        ]
    );
}

#[test]
fn test_poll_latest_score_before_time_and_clear_prior_overflow_wrapped_zero() {
    let mut buffer = TimedScoreRing::<SimpleScore>::from_state(TimedScoreRingState {
        read: 3,
        write: 0,
        len: 2,
        times: vec![0, 0, 0, 1, 2],
        scores: vec![
            None,
            None,
            None,
            Some(SimpleScore::new(1)),
            Some(SimpleScore::new(2)),
        ],
    })
    .unwrap();
    assert_eq!(buffer.retain_floor(10).unwrap().1, &SimpleScore::new(2));
    assert_eq!(buffer.state().read, 4);
    assert_eq!(buffer.state().write, 0);
    assert_eq!(buffer.state().times, vec![0, 0, 0, 0, 2]);
    assert_eq!(
        buffer.state().scores,
        vec![None, None, None, None, Some(SimpleScore::new(2))]
    );
}

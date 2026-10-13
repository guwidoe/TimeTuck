// Adapted full pinned Community TEST parameter rows, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
pub type Row = (
    ExactPhaseKind,
    Option<NodeExploration>,
    bool,
    &'static str,
    Option<&'static [&'static str]>,
);
pub fn rows() -> Vec<Row> {
    vec![
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::DepthFirst),
            false,
            "NONE",
            Some(&[
                "----", "1---", "13--", "132-", "13-2", "12--", "123-", "12-3", "1-3-", "123-",
                "1321",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::DepthFirst),
            false,
            "DESCENDING",
            Some(&[
                "----", "3---", "32--", "321-", "32-1", "31--", "312-", "31-2", "3-2-", "312-",
                "3213",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::DepthFirst),
            false,
            "ASCENDING",
            Some(&[
                "----", "1---", "12--", "123-", "12-3", "13--", "132-", "13-2", "1-2-", "132-",
                "1231",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::DepthFirst),
            true,
            "NONE",
            Some(&[
                "----", "1---", "1-3-", "123-", "1-32", "1-2-", "132-", "1-23", "13--", "132-",
                "1231",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::DepthFirst),
            true,
            "DESCENDING",
            Some(&[
                "----", "3---", "3-2-", "312-", "3-21", "3-1-", "321-", "3-12", "32--", "321-",
                "3123",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::DepthFirst),
            true,
            "ASCENDING",
            Some(&[
                "----", "1---", "1-2-", "132-", "1-23", "1-3-", "123-", "1-32", "12--", "123-",
                "1321",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::BreadthFirst),
            false,
            "NONE",
            Some(&[
                "----", "1---", "3---", "2---", "-1--", "-3--", "-2--", "--1-", "--3-", "--2-",
                "----",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::BreadthFirst),
            false,
            "DESCENDING",
            Some(&[
                "----", "3---", "2---", "1---", "-3--", "-2--", "-1--", "--3-", "--2-", "--1-",
                "----",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::BreadthFirst),
            false,
            "ASCENDING",
            Some(&[
                "----", "1---", "2---", "3---", "-1--", "-2--", "-3--", "--1-", "--2-", "--3-",
                "----",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::BreadthFirst),
            true,
            "NONE",
            Some(&[
                "----", "1---", "3---", "2---", "--1-", "--3-", "--2-", "-1--", "-3--", "-2--",
                "----",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::BreadthFirst),
            true,
            "DESCENDING",
            Some(&[
                "----", "3---", "2---", "1---", "--3-", "--2-", "--1-", "-3--", "-2--", "-1--",
                "----",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::BreadthFirst),
            true,
            "ASCENDING",
            Some(&[
                "----", "1---", "2---", "3---", "--1-", "--2-", "--3-", "-1--", "-2--", "-3--",
                "----",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::ScoreFirst),
            false,
            "NONE",
            Some(&[
                "----", "1---", "13--", "132-", "13-2", "12--", "123-", "12-3", "1-3-", "123-",
                "1321",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::ScoreFirst),
            false,
            "DESCENDING",
            Some(&[
                "----", "3---", "32--", "321-", "32-1", "31--", "312-", "31-2", "3-2-", "312-",
                "3213",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::ScoreFirst),
            false,
            "ASCENDING",
            Some(&[
                "----", "1---", "12--", "123-", "12-3", "13--", "132-", "13-2", "1-2-", "132-",
                "1231",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::ScoreFirst),
            true,
            "NONE",
            Some(&[
                "----", "1---", "1-3-", "123-", "1-32", "1-2-", "132-", "1-23", "13--", "132-",
                "1231",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::ScoreFirst),
            true,
            "DESCENDING",
            Some(&[
                "----", "3---", "3-2-", "312-", "3-21", "3-1-", "321-", "3-12", "32--", "321-",
                "3123",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::ScoreFirst),
            true,
            "ASCENDING",
            Some(&[
                "----", "1---", "1-2-", "132-", "1-23", "1-3-", "123-", "1-32", "12--", "123-",
                "1321",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::OptimisticBoundFirst),
            false,
            "NONE",
            Some(&[
                "----", "1---", "13--", "132-", "13-2", "12--", "123-", "12-3", "1-3-", "123-",
                "1321",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::OptimisticBoundFirst),
            false,
            "DESCENDING",
            Some(&[
                "----", "3---", "32--", "321-", "32-1", "31--", "312-", "31-2", "3-2-", "312-",
                "3213",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::OptimisticBoundFirst),
            false,
            "ASCENDING",
            Some(&[
                "----", "1---", "12--", "123-", "12-3", "13--", "132-", "13-2", "1-2-", "132-",
                "1231",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::OptimisticBoundFirst),
            true,
            "NONE",
            Some(&[
                "----", "1---", "1-3-", "123-", "1-32", "1-2-", "132-", "1-23", "13--", "132-",
                "1231",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::OptimisticBoundFirst),
            true,
            "DESCENDING",
            Some(&[
                "----", "3---", "3-2-", "312-", "3-21", "3-1-", "321-", "3-12", "32--", "321-",
                "3123",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::OptimisticBoundFirst),
            true,
            "ASCENDING",
            Some(&[
                "----", "1---", "1-2-", "132-", "1-23", "1-3-", "123-", "1-32", "12--", "123-",
                "1321",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::InputOrderDepthFirst),
            false,
            "NONE",
            Some(&[
                "----", "1---", "11--", "111-", "113-", "13--", "132-", "13-2", "12--", "123-",
                "1132",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::InputOrderDepthFirst),
            false,
            "DESCENDING",
            Some(&[
                "----", "3---", "33--", "333-", "332-", "32--", "321-", "32-1", "31--", "312-",
                "3321",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::InputOrderDepthFirst),
            false,
            "ASCENDING",
            Some(&[
                "----", "1---", "11--", "111-", "112-", "12--", "123-", "12-3", "13--", "132-",
                "1123",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::InputOrderDepthFirst),
            true,
            "NONE",
            Some(&[
                "----", "1---", "1-1-", "111-", "131-", "1-3-", "123-", "1-32", "1-2-", "132-",
                "1312",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::InputOrderDepthFirst),
            true,
            "DESCENDING",
            Some(&[
                "----", "3---", "3-3-", "333-", "323-", "3-2-", "312-", "3-21", "3-1-", "321-",
                "3231",
            ]),
        ),
        (
            ExactPhaseKind::BranchAndBound,
            Some(NodeExploration::InputOrderDepthFirst),
            true,
            "ASCENDING",
            Some(&[
                "----", "1---", "1-1-", "111-", "121-", "1-2-", "132-", "1-23", "1-3-", "123-",
                "1213",
            ]),
        ),
        (
            ExactPhaseKind::BruteForce,
            None,
            false,
            "NONE",
            Some(&[
                "----", "1---", "11--", "111-", "113-", "112-", "11-1", "11-3", "11-2", "13--",
                "1132",
            ]),
        ),
        (
            ExactPhaseKind::BruteForce,
            None,
            false,
            "DESCENDING",
            Some(&[
                "----", "3---", "33--", "333-", "332-", "331-", "33-3", "33-2", "33-1", "32--",
                "3321",
            ]),
        ),
        (
            ExactPhaseKind::BruteForce,
            None,
            false,
            "ASCENDING",
            Some(&[
                "----", "1---", "11--", "111-", "112-", "113-", "11-1", "11-2", "11-3", "12--",
                "1123",
            ]),
        ),
        (
            ExactPhaseKind::BruteForce,
            None,
            true,
            "NONE",
            Some(&[
                "----", "1---", "1-1-", "111-", "131-", "121-", "1-11", "1-13", "1-12", "1-3-",
                "1312",
            ]),
        ),
        (
            ExactPhaseKind::BruteForce,
            None,
            true,
            "DESCENDING",
            Some(&[
                "----", "3---", "3-3-", "333-", "323-", "313-", "3-33", "3-32", "3-31", "3-2-",
                "3231",
            ]),
        ),
        (
            ExactPhaseKind::BruteForce,
            None,
            true,
            "ASCENDING",
            Some(&[
                "----", "1---", "1-1-", "111-", "121-", "131-", "1-11", "1-12", "1-13", "1-2-",
                "1213",
            ]),
        ),
        (
            ExactPhaseKind::BruteForce,
            Some(NodeExploration::DepthFirst),
            true,
            "DESCENDING",
            None,
        ),
        (
            ExactPhaseKind::BruteForce,
            Some(NodeExploration::BreadthFirst),
            true,
            "DESCENDING",
            None,
        ),
        (
            ExactPhaseKind::BruteForce,
            Some(NodeExploration::ScoreFirst),
            true,
            "DESCENDING",
            None,
        ),
        (
            ExactPhaseKind::BruteForce,
            Some(NodeExploration::OptimisticBoundFirst),
            true,
            "DESCENDING",
            None,
        ),
    ]
}

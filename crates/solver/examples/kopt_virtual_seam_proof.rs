//! TEST/helper and preserved oracle fixture adaptation, Apache-2.0.
use timetuck::*;
fn at(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn main() {
    for (a, b, left, cut, expected) in [
        (
            vec![1, 2, 3, 6],
            vec![4, 5],
            1,
            0,
            vec![vec![1, 5], vec![2, 6, 4, 3]],
        ),
        (
            vec![1, 7, 8, 2, 3, 9, 10, 6],
            vec![4, 5, 11, 12],
            3,
            0,
            vec![vec![1, 5, 11, 12, 2, 8], vec![7, 9, 10, 6, 4, 3]],
        ),
    ] {
        let values = a.iter().chain(&b).copied().collect();
        let p =
            ListProblem::new_indexed(values, vec![ListEntity::new(a), ListEntity::new(b)]).unwrap();
        let mut assignment = p.initial_assignment();
        let tail = assignment.lists()[0].len() - 1;
        let plan = VirtualSeam3OptPlan::prepare(
            &p,
            &assignment,
            &[
                (at(0, tail), at(0, 0)),
                (at(0, left), at(0, left + 1)),
                (at(1, cut), at(1, cut + 1)),
            ],
            &[(1, 3), (2, 5), (4, 0)],
        )
        .unwrap();
        let movement = plan.movement(&p, &assignment).unwrap();
        {
            let trial = p.apply(&mut assignment, &movement).unwrap();
            println!("actual={:?}", trial.lists());
            assert_eq!(trial.lists(), expected);
        }
        assert_eq!(assignment, p.initial_assignment());
    }
}

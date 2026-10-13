//! Native per-session seed selection; no cryptographic or Java RNG contract.
use crate::SolverPhase;
use std::{
    collections::hash_map::RandomState,
    hash::BuildHasher,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

pub(super) fn fresh_seed() -> u64 {
    static SALT: OnceLock<u64> = OnceLock::new();
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let salt = *SALT.get_or_init(|| RandomState::new().hash_one(0_u64));
    let ticket = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .expect("native factory seed sequence exhausted");
    salt.wrapping_add(ticket)
}
pub(super) fn reseed_phase<S>(phase: &mut SolverPhase<'_, S>, seed: u64) {
    match phase {
        SolverPhase::LocalSearch(search) => {
            match &mut search.move_order {
                crate::MoveOrder::Original => {}
                crate::MoveOrder::ShuffledVariables { seed: configured }
                | crate::MoveOrder::ShuffledMoves {
                    seed: configured, ..
                } => *configured = seed,
            }
            if let crate::TieBreaking::Random { seed: configured } = &mut search.tie_breaking {
                *configured = seed;
            }
        }
        SolverPhase::RuinRecreate(search) => search.seed = seed,
        _ => {}
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn independent_threads_choose_distinct_factory_seeds() {
        let threads: Vec<_> = (0..8)
            .map(|_| {
                std::thread::spawn(|| (0..256).map(|_| super::fresh_seed()).collect::<Vec<_>>())
            })
            .collect();
        let mut seeds: Vec<_> = threads
            .into_iter()
            .flat_map(|t| t.join().unwrap())
            .collect();
        seeds.sort_unstable();
        seeds.dedup();
        assert_eq!(seeds.len(), 2048);
    }
}

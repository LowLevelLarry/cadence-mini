// GATE 2c — resilience: finalization completes with f byzantine-silent validators at the
// paper's optimal bound (n = 3f+1); one over the bound (f+1 silent) may stall, but must
// never produce conflicting finalizations (safety still holds).

use crate::chorus_common::*;
use cadence_mini::chorus::ProposerBehavior;
use std::collections::HashMap;

#[test]
fn finalizes_at_f_silent_bound() {
    for seed in 0..20u64 {
        let config = base_config(vec![0]);
        let behavior = HashMap::from([(0, ProposerBehavior::Honest)]);
        let delay = fixed_delay(3);
        // mute exactly f=1 validators (never the proposer, so dissemination still happens)
        let muted = [1u64];
        let mut engine = build_engine_with_muted(config, behavior, delay, seed, &muted);
        engine.start();
        engine.run_until(300);

        for node in engine.nodes() {
            if muted.contains(&node.id) {
                continue;
            }
            assert!(
                node.finalized.is_some(),
                "seed {seed}: honest validator {} failed to finalize with f=1 silent validators",
                node.id
            );
        }
    }
}

#[test]
fn stalls_safely_beyond_bound() {
    // f+1 = 2 silent validators at n=4,f=1: one over the optimal bound. May stall, must
    // never violate safety (no two honest validators finalize differently).
    for seed in 0..20u64 {
        let config = base_config(vec![0]);
        let behavior = HashMap::from([(0, ProposerBehavior::Honest)]);
        let delay = fixed_delay(3);
        let muted = [1u64, 2u64];
        let mut engine = build_engine_with_muted(config, behavior, delay, seed, &muted);
        engine.start();
        engine.run_until(300);

        let mut finals = Vec::new();
        for node in engine.nodes() {
            if let Some(f) = &node.finalized {
                finals.push(f.meta_block.clone());
            }
        }
        if let Some(first) = finals.first() {
            assert!(
                finals.iter().all(|mb| mb == first),
                "seed {seed}: safety violated beyond the resilience bound"
            );
        }
    }
}

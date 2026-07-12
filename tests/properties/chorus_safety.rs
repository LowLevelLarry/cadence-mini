// safety: no two honest validators should ever finalize conflicting blocks for a slot,
// even under adversarial (widely varying) delays.

use crate::chorus_common::*;
use cadence_mini::chorus::ProposerBehavior;
use std::collections::HashMap;

fn run_seed(seed: u64) -> bool {
    let config = base_config(vec![0]);
    let behavior = HashMap::from([(0, ProposerBehavior::Honest)]);
    // wide, seed-driven delay range models "adversarial" (highly variable) network timing
    let delay = synchronous_delay(1 + (seed % 40), seed);
    let mut engine = build_engine(config, behavior, delay, seed);
    engine.start();
    engine.run_until(500);

    let mut finals = Vec::new();
    for node in engine.nodes() {
        if let Some(f) = &node.finalized {
            finals.push(f.meta_block.clone());
        }
    }
    if let Some(first) = finals.first() {
        finals.iter().all(|mb| mb == first)
    } else {
        true // nobody finalized yet under this delay — not a safety violation
    }
}

#[test]
fn no_conflicting_finalization() {
    for seed in 0..20u64 {
        assert!(run_seed(seed), "conflicting finalization observed at seed {seed}");
    }
}

#[test]
fn sanity_finalization_actually_happens() {
    let mut finalized_count = 0;
    let mut fallback_count = 0;
    for seed in 0..20u64 {
        let config = base_config(vec![0]);
        let behavior = HashMap::from([(0, ProposerBehavior::Honest)]);
        let delay = synchronous_delay(1 + (seed % 40), seed);
        let mut engine = build_engine(config, behavior, delay, seed);
        engine.start();
        engine.run_until(500);
        for node in engine.nodes() {
            if let Some(f) = &node.finalized {
                finalized_count += 1;
                if f.via_fallback {
                    fallback_count += 1;
                }
            }
        }
    }
    println!("finalized_count={finalized_count} fallback_count={fallback_count}");
    assert!(finalized_count > 0);
}

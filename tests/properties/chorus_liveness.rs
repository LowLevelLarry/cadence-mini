// slot consensus termination (§3.1): every honest validator eventually finalizes a block.

use crate::chorus_common::*;
use cadence_mini::chorus::ProposerBehavior;
use std::collections::HashMap;

#[test]
fn always_terminates() {
    for seed in 0..20u64 {
        let config = base_config(vec![0]);
        let behavior = HashMap::from([(0, ProposerBehavior::Honest)]);
        let delay = synchronous_delay(1 + (seed % 15), seed);
        let mut engine = build_engine(config, behavior, delay, seed);
        engine.start();
        engine.run_until(1000);

        for node in engine.nodes() {
            assert!(
                node.finalized.is_some(),
                "seed {seed}: validator {} never terminated under synchrony",
                node.id
            );
        }
    }
}

// speculative soundness: speculative finality should never be reverted in a run
// lacking an equivocation.

use crate::chorus_common::*;
use cadence_mini::chorus::ProposerBehavior;
use cadence_mini::speculative::{outcome, SpeculativeOutcome};
use std::collections::HashMap;

#[test]
fn no_revert_without_equivocation() {
    for seed in 0..20u64 {
        let config = base_config(vec![0, 1]);
        let behavior = HashMap::from([
            (0, ProposerBehavior::Honest),
            (1, ProposerBehavior::Honest),
        ]);
        let delay = synchronous_delay(1 + (seed % 30), seed);
        let mut engine = build_engine(config, behavior, delay, seed);
        engine.start();
        engine.run_until(600);

        for node in engine.nodes() {
            if let SpeculativeOutcome::Reverted { speculative, finalized } = outcome(node) { panic!(
                "seed {seed}: validator {} reverted without any equivocating proposer: {:?} -> {:?}",
                node.id, speculative, finalized
            ) }
        }
    }
}

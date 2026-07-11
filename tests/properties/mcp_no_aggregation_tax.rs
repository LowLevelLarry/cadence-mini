// GATE 4c — no aggregation tax: fast-path round count with k proposers equals the
// single-proposer (M2) round count (2: round-1 vote, fast vote). MCP is folded into the
// same two rounds, not bolted on as an extra aggregation phase.

use crate::chorus_common::*;
use cadence_mini::chorus::ProposerBehavior;
use std::collections::HashMap;

#[test]
fn round_count_independent_of_k() {
    for k in [1usize, 2, 3] {
        let proposers: Vec<u64> = (0..k as u64).collect();
        let config = base_config(proposers.clone());
        let behavior: HashMap<u64, ProposerBehavior> =
            proposers.iter().map(|&p| (p, ProposerBehavior::Honest)).collect();
        let delay = fixed_delay(2);
        let mut engine = build_engine(config, behavior, delay, 5 + k as u64);
        engine.start();
        engine.run_until(200);

        for node in engine.nodes() {
            let finalized = node
                .finalized
                .as_ref()
                .unwrap_or_else(|| panic!("k={k}: validator {} never finalized", node.id));
            assert!(!finalized.via_fallback, "k={k}: unexpectedly fell back to the slower path");
            assert_eq!(
                finalized.round, 2,
                "k={k}: fast-path round count should stay 2 regardless of proposer count"
            );
        }
    }
}

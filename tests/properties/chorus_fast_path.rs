// fast path: all-honest validators under synchrony should finalize in exactly 3
// communication rounds (dissemination, round-1 vote, fast vote).

use crate::chorus_common::*;
use cadence_mini::chorus::ProposerBehavior;
use std::collections::HashMap;

#[test]
fn fast_path_three_rounds() {
    let config = base_config(vec![0]);
    let behavior = HashMap::from([(0, ProposerBehavior::Honest)]);
    let delay = fixed_delay(2); // small, synchronous delay
    let mut engine = build_engine(config, behavior, delay, 99);
    engine.start();
    engine.run_until(200);

    for node in engine.nodes() {
        let finalized = node
            .finalized
            .as_ref()
            .unwrap_or_else(|| panic!("validator {} never finalized under synchrony", node.id));
        assert!(!finalized.via_fallback, "fast path should not fall back when all-honest + synchronous");
        assert_eq!(finalized.round, 2, "fast path finality lands at round 2 (round-1 vote, fast vote)");

        let spec = node
            .speculative
            .as_ref()
            .expect("speculative finality should have been reached one round earlier");
        assert_eq!(spec.round, 1);
        assert_eq!(spec.meta_block, finalized.meta_block, "no equivocation, so speculative == final");
    }
}

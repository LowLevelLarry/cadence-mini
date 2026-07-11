// GATE 4a — censorship resistance: a transaction submitted to exactly one correct proposer
// appears in the finalized slot's block under synchrony, even when every other proposer
// actively "censors" it (deliberately builds a proposal without it), 20 seeds.

use crate::chorus_common::*;
use cadence_mini::chorus::ProposerBehavior;
use std::collections::HashMap;

const TARGET_TX: u64 = 424242;

#[test]
fn single_honest_proposer_tx_included() {
    for seed in 0..20u64 {
        let proposers = vec![0u64, 1, 2];
        let config = base_config(proposers.clone());
        let mut behavior = HashMap::new();
        // proposer 0 is the one correct proposer that includes the target transaction
        behavior.insert(0, ProposerBehavior::Censor { payload: vec![TARGET_TX] });
        // proposers 1 and 2 actively censor: they build proposals that never contain it
        behavior.insert(1, ProposerBehavior::Censor { payload: vec![111, 112] });
        behavior.insert(2, ProposerBehavior::Censor { payload: vec![222, 223] });

        let delay = synchronous_delay(1 + (seed % 5), seed);
        let mut engine = build_engine(config, behavior, delay, seed);
        engine.start();
        engine.run_until(500);

        for node in engine.nodes() {
            let block = node
                .recovered_block()
                .unwrap_or_else(|| panic!("seed {seed}: validator {} never finalized/recovered a block", node.id));
            assert!(
                block.txs.contains(&TARGET_TX),
                "seed {seed}: validator {} finalized block without the honest proposer's transaction: {:?}",
                node.id,
                block.txs
            );
        }
    }
}

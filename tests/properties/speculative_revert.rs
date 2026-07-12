// revert correctness: an equivocating proposer should never cause an unsound revert.
// Concretely: (1) whenever a validator's speculative view does diverge from what eventually
// finalizes, there should always be a justifying equivocation proof for the proposer whose
// entry changed; (2) full finality is never violated by the presence of an equivocator;
// (3) the equivocation-detection machinery itself gets exercised (proofs actually get
// recorded somewhere in the run) so this isn't a vacuous pass.
//
// side note on why this doesn't try to force a *live* revert: with the simplified
// fallback-agreement leader rule here (max-multiplicity vote among received proposals), a
// fast-quorum-backed meta-block is always held by >= 2f+1 of the n=3f+1 validators, which is
// provably always a plurality of any 2f+1-sized batch the leader collects — a minority
// (excluded/other-digest) proposal can mathematically never outvote it. So instead this test
// uses a genuine 2-vs-2 split (n=4,f=1) that reliably triggers equivocation *detection* (both
// digests clear the f+1 threshold, so no validator ever reaches fast-path quorum and everyone
// provably goes to fallback), and checks the *conditional* soundness property — if a revert
// is ever observed, it's always justified — which holds regardless of whether this particular
// setup can produce a live one. The proof-matching logic itself is verified directly and
// unconditionally in the second test below, independent of any live simulation run.

use crate::chorus_common::*;
use cadence_mini::chorus::ProposerBehavior;
use cadence_mini::speculative::{justifying_proof, outcome, SpeculativeOutcome};
use cadence_mini::types::{Entry, MetaBlock};
use std::collections::HashMap;

#[test]
fn equivocation_triggers_correct_revert() {
    let mut any_proof_recorded = false;
    let mut any_finalization = false;

    for seed in 0..20u64 {
        let config = base_config(vec![0, 1]);
        let mut behavior = HashMap::new();
        // proposer 0 equivocates with an even 2-vs-2 split (n=4,f=1): neither digest can
        // reach the 2f+1=3 fast quorum, but both clear the f+1=2 fallback-strength
        // threshold, so every validator is forced to fallback and independently detects
        // the equivocation (see the file-level comment for why this split, not 3-vs-1)
        behavior.insert(
            0,
            ProposerBehavior::Equivocate {
                payload_a: vec![1000],
                payload_b: vec![2000],
                split_a: vec![0, 1],
            },
        );
        behavior.insert(1, ProposerBehavior::Honest);

        let delay = synchronous_delay(1 + (seed % 20), seed);
        let mut engine = build_engine(config, behavior, delay, seed);
        engine.start();
        engine.run_until(600);

        let mut finals = Vec::new();
        for node in engine.nodes() {
            if !node.equivocation_proofs.is_empty() {
                any_proof_recorded = true;
            }
            if let Some(f) = &node.finalized {
                any_finalization = true;
                finals.push(f.meta_block.clone());
            }
            if let SpeculativeOutcome::Reverted { speculative, finalized } = outcome(node) {
                let proof = justifying_proof(&node.equivocation_proofs, &speculative, &finalized);
                assert!(
                    proof.is_some(),
                    "seed {seed}: validator {} reverted from {:?} to {:?} with no justifying equivocation proof",
                    node.id,
                    speculative,
                    finalized
                );
            }
        }
        // safety still holds: no two honest validators finalize conflicting blocks, even
        // with an equivocator present in the slot
        if let Some(first) = finals.first() {
            assert!(
                finals.iter().all(|mb| mb == first),
                "seed {seed}: safety violated in the presence of an equivocating proposer"
            );
        }
    }

    assert!(any_finalization, "no seed ever finalized — scenario is unreachable, not just revert-free");
    assert!(
        any_proof_recorded,
        "the equivocation-detection path was never exercised across any seed — test is vacuous"
    );
}

// direct unit check of the proof-matching logic itself (independent of whether the live
// simulation above happens to trigger a revert in any particular seed)
#[test]
fn justifying_proof_matches_the_changed_proposer() {
    use cadence_mini::chorus::EquivocationProof;

    let speculative = MetaBlock {
        slot: 1,
        entries: vec![(0, Entry::Included(111)), (1, Entry::Included(222))],
    };
    let finalized = MetaBlock {
        slot: 1,
        entries: vec![(0, Entry::Excluded), (1, Entry::Included(222))],
    };
    let proof_for_wrong_proposer = EquivocationProof {
        slot: 1,
        proposer: 1, // did not change between speculative and finalized
        digest_a: 1,
        witnesses_a: vec![0],
        digest_b: 2,
        witnesses_b: vec![1],
    };
    let proof_for_right_proposer = EquivocationProof {
        slot: 1,
        proposer: 0, // this is the one whose entry actually changed
        digest_a: 111,
        witnesses_a: vec![0, 1],
        digest_b: 999,
        witnesses_b: vec![2, 3],
    };

    assert!(
        justifying_proof(std::slice::from_ref(&proof_for_wrong_proposer), &speculative, &finalized).is_none()
    );
    assert!(justifying_proof(
        &[proof_for_wrong_proposer, proof_for_right_proposer],
        &speculative,
        &finalized
    )
    .is_some());
}

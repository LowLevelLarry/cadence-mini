// GATE 3c — throttling (boundedness): under injected instability (delays spiking 10x),
// outstanding instances never exceed the configured bound (paper: at most 2W - p).

use cadence_mini::chorus::ProposerBehavior;
use cadence_mini::pipeline::{ConductorConfig, PipelineValidator};
use cadence_mini::sim::{Engine, FixedDelay};
use cadence_mini::types::{ProposerId, Slot, ValidatorId};
use std::collections::HashMap;

fn one_proposer_per_slot(cfg: &ConductorConfig, slot: Slot) -> Vec<ProposerId> {
    vec![cfg.validators[(slot as usize) % cfg.validators.len()]]
}

fn honest(_id: ValidatorId) -> HashMap<ProposerId, ProposerBehavior> {
    HashMap::new()
}

#[test]
fn outstanding_never_exceeds_bound() {
    let validators: Vec<ValidatorId> = (0..4).collect();
    let window = 10usize;
    let threshold = 3usize;
    let cfg = ConductorConfig {
        n: 4,
        f: 1,
        delta: 5,
        fallback_timeout: 60,
        tau: 5,
        window,
        threshold,
        validators: validators.clone(),
    };
    // a sustained 10x delay spike relative to a "normal" 5-tick baseline models the
    // instability the gate asks for: the network never stabilizes within the run
    let mut engine: Engine<PipelineValidator> = Engine::new(3, Box::new(FixedDelay(50)));
    for &id in &validators {
        engine.add_node(PipelineValidator::new(
            id,
            cfg.clone(),
            one_proposer_per_slot,
            honest,
            Default::default(),
        ));
    }
    engine.start();
    engine.run_until(5000);

    let bound = 2 * window - threshold;
    for node in engine.nodes() {
        assert!(
            node.max_outstanding <= bound,
            "validator {} exceeded the boundedness bound: max_outstanding={} > 2W-p={}",
            node.id,
            node.max_outstanding,
            bound
        );
    }
}

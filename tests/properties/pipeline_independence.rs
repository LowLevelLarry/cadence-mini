// GATE 3a — independence: with 100-tick link delays and a 10-tick block interval, the trace
// shows >= 5 instances simultaneously in flight (pipelining is real, not sequential).

use cadence_mini::pipeline::{ConductorConfig, PipelineValidator};
use cadence_mini::sim::{Engine, FixedDelay};
use cadence_mini::types::{ProposerId, Slot, ValidatorId};
use std::collections::HashMap;

fn one_proposer_per_slot(cfg: &ConductorConfig, slot: Slot) -> Vec<ProposerId> {
    vec![cfg.validators[(slot as usize) % cfg.validators.len()]]
}

fn honest(_id: ValidatorId) -> HashMap<ProposerId, cadence_mini::chorus::ProposerBehavior> {
    HashMap::new()
}

#[test]
fn five_concurrent_instances() {
    let validators: Vec<ValidatorId> = (0..4).collect();
    let cfg = ConductorConfig {
        n: 4,
        f: 1,
        delta: 5,
        fallback_timeout: 400,
        tau: 10,
        window: 20,
        threshold: 0, // p=0: open the next window without waiting on the current one at all
        validators: validators.clone(),
    };
    let mut engine: Engine<PipelineValidator> = Engine::new(1, Box::new(FixedDelay(100)));
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
    engine.run_until(400);

    let max_outstanding = engine.nodes().map(|n| n.max_outstanding).max().unwrap();
    assert!(
        max_outstanding >= 5,
        "expected >= 5 concurrently in-flight instances, got {max_outstanding}"
    );
}

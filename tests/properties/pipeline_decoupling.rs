// decoupling: halving the block interval should roughly double finalized-blocks-per-
// 1000-ticks, while per-block finalization latency (finalize tick minus deadline) stays flat.

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

fn run(tau: u64, horizon: u64) -> (usize, f64) {
    let validators: Vec<ValidatorId> = (0..4).collect();
    let cfg = ConductorConfig {
        n: 4,
        f: 1,
        delta: 5,
        fallback_timeout: 100,
        tau,
        window: 50,
        threshold: 25,
        validators: validators.clone(),
    };
    let mut engine: Engine<PipelineValidator> = Engine::new(7, Box::new(FixedDelay(5)));
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
    engine.run_until(horizon);

    let node = engine.node(0).unwrap();
    let count = node.finalize_log.len();
    let avg_latency: f64 = if count == 0 {
        0.0
    } else {
        node.finalize_log
            .iter()
            .map(|&(slot, tick)| {
                let deadline = cfg.delta + (slot - 1) * cfg.tau;
                (tick - deadline) as f64
            })
            .sum::<f64>()
            / count as f64
    };
    (count, avg_latency)
}

#[test]
fn halving_interval_doubles_throughput() {
    let horizon = 2000;
    let (count_20, latency_20) = run(20, horizon);
    let (count_10, latency_10) = run(10, horizon);

    assert!(count_20 > 0 && count_10 > 0, "both runs must finalize something");

    let ratio = count_10 as f64 / count_20 as f64;
    assert!(
        (1.5..=2.5).contains(&ratio),
        "expected roughly 2x throughput from halving tau, got ratio {ratio} (count_20={count_20}, count_10={count_10})"
    );

    // per-block latency should stay roughly flat regardless of tau
    let latency_ratio = latency_10.max(1.0) / latency_20.max(1.0);
    assert!(
        (0.5..=2.0).contains(&latency_ratio),
        "expected roughly flat per-block latency, got {latency_10} vs {latency_20}"
    );
}

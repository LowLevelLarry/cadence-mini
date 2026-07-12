// the full adversary suite (equivocator, censor, offline, partition-and-heal) should
// preserve safety in every scenario: no two honest validators ever finalize
// conflicting blocks for a slot.

use cadence_mini::adversary::{censor, delayer, equivocator, offline};
use cadence_mini::chorus::validator::ChorusValidator;
use cadence_mini::chorus::{ChorusConfig, ProposerBehavior};
use cadence_mini::sim::{Engine, FixedDelay};
use cadence_mini::types::ValidatorId;
use std::collections::HashMap;

const N: usize = 4;
const F: usize = 1;

fn config(proposers: Vec<ValidatorId>) -> ChorusConfig {
    ChorusConfig {
        slot: 1,
        deadline: 20,
        delta: 10,
        fallback_timeout: 40,
        proposers,
        validators: (0..N as u64).collect(),
        n: N,
        f: F,
    }
}

fn assert_safe(engine: &Engine<ChorusValidator>, label: &str, seed: u64) {
    let mut finals = Vec::new();
    for node in engine.nodes() {
        if let Some(f) = &node.finalized {
            finals.push(f.meta_block.clone());
        }
    }
    if let Some(first) = finals.first() {
        assert!(
            finals.iter().all(|mb| mb == first),
            "{label}: safety violated at seed {seed}"
        );
    }
}

#[test]
fn safety_holds_under_equivocator() {
    for seed in 0..20u64 {
        let cfg = config(vec![0, 1]);
        let validators = cfg.validators.clone();
        let mut behavior = HashMap::new();
        behavior.insert(0, equivocator::equivocate(vec![1], vec![2], equivocator::half_split(&validators)));
        behavior.insert(1, ProposerBehavior::Honest);

        let mut engine: Engine<ChorusValidator> = Engine::new(seed, Box::new(FixedDelay(1 + seed % 10)));
        for &id in &validators {
            engine.add_node(ChorusValidator::new(id, cfg.clone(), behavior.clone()));
        }
        engine.start();
        engine.run_until(500);
        assert_safe(&engine, "equivocator", seed);
    }
}

#[test]
fn safety_holds_under_censor() {
    for seed in 0..20u64 {
        let cfg = config(vec![0, 1]);
        let mut behavior = HashMap::new();
        behavior.insert(0, censor::censor(vec![10, 11]));
        behavior.insert(1, censor::censor(vec![20, 21]));

        let mut engine: Engine<ChorusValidator> = Engine::new(seed, Box::new(FixedDelay(1 + seed % 10)));
        for &id in &cfg.validators.clone() {
            engine.add_node(ChorusValidator::new(id, cfg.clone(), behavior.clone()));
        }
        engine.start();
        engine.run_until(500);
        assert_safe(&engine, "censor", seed);
    }
}

#[test]
fn safety_holds_under_offline() {
    for seed in 0..20u64 {
        let cfg = config(vec![0]);
        let behavior = HashMap::from([(0, ProposerBehavior::Honest)]);
        let muted = offline::offline_set(&cfg.validators, F);

        let mut engine: Engine<ChorusValidator> = Engine::new(seed, Box::new(FixedDelay(1 + seed % 10)));
        for &id in &cfg.validators.clone() {
            let is_muted = muted.contains(&id);
            engine.add_node(ChorusValidator::new_with_muted(id, cfg.clone(), behavior.clone(), is_muted));
        }
        engine.start();
        engine.run_until(500);
        assert_safe(&engine, "offline", seed);
    }
}

#[test]
fn safety_holds_under_partition_heal() {
    for seed in 0..20u64 {
        let cfg = config(vec![0, 1]);
        let behavior = HashMap::from([(0, ProposerBehavior::Honest), (1, ProposerBehavior::Honest)]);
        // asynchronous for a while, then heals partway through the run
        let delay = delayer::partition_then_heal(150 + seed % 50, 500, 2);

        let mut engine: Engine<ChorusValidator> = Engine::new(seed, Box::new(delay));
        for &id in &cfg.validators.clone() {
            engine.add_node(ChorusValidator::new(id, cfg.clone(), behavior.clone()));
        }
        engine.start();
        engine.run_until(3000);
        assert_safe(&engine, "partition_heal", seed);
    }
}

// shared harness for the Chorus (M2) property tests — single slot, single proposer.

use cadence_mini::chorus::validator::ChorusValidator;
use cadence_mini::chorus::{ChorusConfig, ChorusMsg, ProposerBehavior};
use cadence_mini::sim::{DelayModel, Engine, FixedDelay, OverrideDelay, UniformDelay};
use std::collections::HashMap;

pub const N: usize = 4;
pub const F: usize = 1;
pub const DELTA: u64 = 10;
pub const DEADLINE: u64 = 20;
pub const FALLBACK_TIMEOUT: u64 = 30;

pub fn base_config(proposers: Vec<u64>) -> ChorusConfig {
    ChorusConfig {
        slot: 1,
        deadline: DEADLINE,
        delta: DELTA,
        fallback_timeout: FALLBACK_TIMEOUT,
        proposers,
        validators: (0..N as u64).collect(),
        n: N,
        f: F,
    }
}

pub fn build_engine(
    config: ChorusConfig,
    behavior: HashMap<u64, ProposerBehavior>,
    delay: Box<dyn DelayModel>,
    run_seed: u64,
) -> Engine<ChorusValidator> {
    build_engine_with_muted(config, behavior, delay, run_seed, &[])
}

pub fn build_engine_with_muted(
    config: ChorusConfig,
    behavior: HashMap<u64, ProposerBehavior>,
    delay: Box<dyn DelayModel>,
    run_seed: u64,
    muted: &[u64],
) -> Engine<ChorusValidator> {
    let mut engine: Engine<ChorusValidator> = Engine::new(run_seed, delay);
    for &id in &config.validators.clone() {
        let is_muted = muted.contains(&id);
        engine.add_node(ChorusValidator::new_with_muted(
            id,
            config.clone(),
            behavior.clone(),
            is_muted,
        ));
    }
    engine
}

pub fn synchronous_delay(max: u64, seed: u64) -> Box<dyn DelayModel> {
    let _ = seed;
    Box::new(UniformDelay { min: 1, max })
}

pub fn fixed_delay(d: u64) -> Box<dyn DelayModel> {
    Box::new(FixedDelay(d))
}

#[allow(dead_code)]
pub fn override_delay(base: u64, overrides: HashMap<(u64, u64), u64>) -> Box<dyn DelayModel> {
    Box::new(OverrideDelay {
        base: FixedDelay(base),
        overrides,
    })
}

#[allow(dead_code)]
pub fn unused_msg_marker(_m: ChorusMsg) {}

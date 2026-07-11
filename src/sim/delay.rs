use crate::sim::rng::link_rng;
use rand::RngExt;
use std::collections::HashMap;

pub type NodeId = u64;
pub type Tick = u64;

// injectable per-link message delay. deterministic given (run_seed, from, to, send_seq) —
// send_seq is a per-link counter so repeated sends on the same link don't all draw the
// same delay, while replaying the same seed still reproduces the same sequence of delays.
// `now` is the send tick, so a model can vary delay over time (e.g. asynchronous-until-GST).
pub trait DelayModel {
    fn delay(&mut self, run_seed: u64, from: NodeId, to: NodeId, send_seq: u64, now: Tick) -> Tick;
}

pub struct FixedDelay(pub Tick);

impl DelayModel for FixedDelay {
    fn delay(&mut self, _run_seed: u64, _from: NodeId, _to: NodeId, _send_seq: u64, _now: Tick) -> Tick {
        self.0
    }
}

pub struct UniformDelay {
    pub min: Tick,
    pub max: Tick,
}

impl DelayModel for UniformDelay {
    fn delay(&mut self, run_seed: u64, from: NodeId, to: NodeId, send_seq: u64, _now: Tick) -> Tick {
        let mut rng = link_rng(run_seed ^ send_seq.wrapping_mul(0x9E37_79B9), from, to);
        if self.min >= self.max {
            self.min
        } else {
            rng.random_range(self.min..=self.max)
        }
    }
}

// per-(from,to) override, falling back to a base model; lets tests spike specific links
pub struct OverrideDelay<B: DelayModel> {
    pub base: B,
    pub overrides: HashMap<(NodeId, NodeId), Tick>,
}

impl<B: DelayModel> DelayModel for OverrideDelay<B> {
    fn delay(&mut self, run_seed: u64, from: NodeId, to: NodeId, send_seq: u64, now: Tick) -> Tick {
        if let Some(&d) = self.overrides.get(&(from, to)) {
            d
        } else {
            self.base.delay(run_seed, from, to, send_seq, now)
        }
    }
}

// partial synchrony, made literal: arbitrary (here, a fixed large bound) delay before
// `heals_at`, the paper's GST, and a small bounded delay from then on. used by the
// partition-and-heal adversary (M5).
pub struct PartitionHeal {
    pub heals_at: Tick,
    pub asynchronous_delay: Tick,
    pub synchronous_delay: Tick,
}

impl DelayModel for PartitionHeal {
    fn delay(&mut self, _run_seed: u64, _from: NodeId, _to: NodeId, _send_seq: u64, now: Tick) -> Tick {
        if now < self.heals_at {
            self.asynchronous_delay
        } else {
            self.synchronous_delay
        }
    }
}

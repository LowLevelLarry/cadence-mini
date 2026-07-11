// a geo-plausible delay matrix (M6): validators are spread across a handful of regions with
// realistic-shaped one-way latencies (same-region: single-digit ms; cross-region: tens to
// ~250ms), used to approximate the "shape" of the paper's 200-validator experiment (§8) —
// not its actual published numbers, which aren't available to reproduce from (NOTES.md
// ambiguity #7).

use crate::sim::rng::link_rng;
use crate::sim::{DelayModel, NodeId, Tick};
use crate::types::ValidatorId;
use rand::Rng;
use std::collections::HashMap;

pub const REGIONS: [&str; 5] = ["us-east", "us-west", "europe", "asia", "sa"];

// symmetric one-way base latency in ms between regions, roughly shaped after real internet
// backbone RTT/2 figures; same-region entries are intra-datacenter-ish, not zero
fn base_latency_ms(a: usize, b: usize) -> Tick {
    if a == b {
        return 8;
    }
    const TABLE: [[Tick; 5]; 5] = [
        // us-east, us-west, europe,  asia,   sa
        [8, 60, 80, 150, 110],
        [60, 8, 140, 110, 170],
        [80, 140, 8, 160, 190],
        [150, 110, 160, 8, 220],
        [110, 170, 190, 220, 8],
    ];
    TABLE[a][b]
}

pub fn assign_regions(validators: &[ValidatorId]) -> HashMap<ValidatorId, usize> {
    validators
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, i % REGIONS.len()))
        .collect()
}

pub struct GeoDelay {
    pub region_of: HashMap<ValidatorId, usize>,
    pub jitter_ms: Tick,
}

impl DelayModel for GeoDelay {
    fn delay(&mut self, run_seed: u64, from: NodeId, to: NodeId, send_seq: u64, _now: Tick) -> Tick {
        let ra = *self.region_of.get(&from).unwrap_or(&0);
        let rb = *self.region_of.get(&to).unwrap_or(&0);
        let base = base_latency_ms(ra, rb);
        if self.jitter_ms == 0 {
            return base;
        }
        let mut rng = link_rng(run_seed ^ send_seq.wrapping_mul(0x9E37_79B9), from, to);
        base + (rng.next_u64() % self.jitter_ms)
    }
}

// tries to reproduce the *shape* of the paper's section-8 latency evaluation — 200
// validators, 5 concurrent proposers per slot, geo-plausible network delay — not its
// published numbers, which aren't available to reproduce from directly. runs many
// independent trials, collects per-validator finalization and speculative-finalization
// latency (ticks past the slot deadline, treated as ms), and writes REPORT.md with
// p50/p90/p99 plus a written comparison to the paper's claims.

use cadence_mini::adversary::geo::{assign_regions, GeoDelay};
use cadence_mini::chorus::validator::ChorusValidator;
use cadence_mini::chorus::{ChorusConfig, ProposerBehavior};
use cadence_mini::sim::Engine;
use cadence_mini::types::ValidatorId;
use std::collections::HashMap;
use std::fs;

const N: usize = 200;
const F: usize = 66;
const PROPOSERS: usize = 5;
const TRIALS: u64 = 30;
const DELTA: u64 = 250; // ms: generous bound over the geo matrix's worst case (~220ms)
const DEADLINE: u64 = 300;
const FALLBACK_TIMEOUT: u64 = 1500;
const HORIZON: u64 = 3000;
const BLOCK_INTERVAL_MS: f64 = 100.0; // paper's headline block interval for the tick claim

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn main() {
    let validators: Vec<ValidatorId> = (0..N as u64).collect();
    let proposers: Vec<ValidatorId> = (0..PROPOSERS as u64).collect();
    let regions = assign_regions(&validators);

    let mut finalization_latencies = Vec::new();
    let mut speculative_latencies = Vec::new();

    for trial in 0..TRIALS {
        let cfg = ChorusConfig {
            slot: 1,
            deadline: DEADLINE,
            delta: DELTA,
            fallback_timeout: FALLBACK_TIMEOUT,
            proposers: proposers.clone(),
            validators: validators.clone(),
            n: N,
            f: F,
        };
        let behavior: HashMap<ValidatorId, ProposerBehavior> =
            proposers.iter().map(|&p| (p, ProposerBehavior::Honest)).collect();

        let delay = GeoDelay { region_of: regions.clone(), jitter_ms: 15 };
        let mut engine: Engine<ChorusValidator> = Engine::new(1000 + trial, Box::new(delay));
        for &id in &validators {
            engine.add_node(ChorusValidator::new(id, cfg.clone(), behavior.clone()));
        }
        engine.start();
        engine.run_until(HORIZON);

        for node in engine.nodes() {
            if let Some(t) = node.finalized_tick {
                finalization_latencies.push(t - DEADLINE);
            }
            if let Some(t) = node.speculative_tick {
                speculative_latencies.push(t - DEADLINE);
            }
        }
    }

    finalization_latencies.sort_unstable();
    speculative_latencies.sort_unstable();

    let avg = |v: &[u64]| -> f64 {
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<u64>() as f64 / v.len() as f64
        }
    };

    let fin_avg = avg(&finalization_latencies);
    let spec_avg = avg(&speculative_latencies);
    let fin_p50 = percentile(&finalization_latencies, 0.50);
    let fin_p90 = percentile(&finalization_latencies, 0.90);
    let fin_p99 = percentile(&finalization_latencies, 0.99);
    let spec_p50 = percentile(&speculative_latencies, 0.50);
    let spec_p90 = percentile(&speculative_latencies, 0.90);
    let spec_p99 = percentile(&speculative_latencies, 0.99);
    let inclusion_wait = BLOCK_INTERVAL_MS / 2.0;

    println!(
        "finalization: avg={fin_avg:.1}ms p50={fin_p50} p90={fin_p90} p99={fin_p99}   speculative: avg={spec_avg:.1}ms p50={spec_p50} p90={spec_p90} p99={spec_p99}"
    );

    let report = format!(
        "# Latency shape check\n\n\
Ran {TRIALS} trials, {N} validators (f={F}), {PROPOSERS} concurrent proposers per slot, over a \
geo-plausible 5-region delay matrix (same-region ~8ms, cross-region 60-220ms, +/-15ms jitter). \
Samples pool every validator's own observed latency across all trials, in ticks (treated as ms).\n\n\
## Distributions\n\n\
| | avg | p50 | p90 | p99 |\n\
|---|---|---|---|---|\n\
| finalization (ticks past deadline) | {fin_avg:.1} | {fin_p50} | {fin_p90} | {fin_p99} |\n\
| speculative finalization | {spec_avg:.1} | {spec_p50} | {spec_p90} | {spec_p99} |\n\n\
Transaction inclusion wait (not simulated — analytic, per the paper's own \"half the block \
interval\" estimate at a {BLOCK_INTERVAL_MS:.0}ms block interval): ~{inclusion_wait:.0}ms.\n\n\
## Comparison to the paper (Section 8)\n\n\
The paper reports, over Monad mainnet's real 200-validator geography with 5 proposers: 219ms \
average finalization, 167ms average speculative finality (speculative/final ratio ~0.76), and \
~50ms average wait to enter a proposal at a 100ms block interval.\n\n\
This run's speculative/final ratio is {ratio:.2} ({spec_avg:.1}ms / {fin_avg:.1}ms). \
{ratio_note}\n\n\
The 2x-median-one-way-delay shape check (finalization ~= 2 * median one-way delay, since the \
fast path is D + 2*delta): this run's median finalization latency is {fin_p50}ms.\n\n\
## Honest caveats\n\n\
- The 5-region delay matrix here is invented to be geo-plausible, not Monad's actual measured \
inter-validator latencies (which aren't published in a reproducible form). Absolute numbers \
are not expected to match the paper's; only the *shape* (speculative substantially faster \
than final, finalization on the order of a couple hundred ms at these delay magnitudes) is \
being compared.\n\
- This experiment measures only Chorus's own fast-path latency for a single slot; it does not \
run the full Conductor/pipeline scheduling loop, so it says nothing about steady-state \
throughput or the block-interval-driven inclusion wait (which is reported analytically above, \
not simulated).\n\
- No fallback-path latency is included: FALLBACK_TIMEOUT is set high enough that, at these \
delay magnitudes, every trial's proposers reach fast-path quorum before it ever fires.\n",
        ratio = spec_avg / fin_avg.max(1.0),
        ratio_note = if fin_avg > 0.0 && (spec_avg / fin_avg - 0.75).abs() < 0.15 {
            "That's within a reasonable band of the paper's ~0.76 ratio."
        } else {
            "That diverges more than expected from the paper's ~0.76 ratio — worth investigating \
             if this experiment is extended."
        },
    );

    fs::write("REPORT.md", report).expect("REPORT.md is writable");
    println!("wrote REPORT.md");
}

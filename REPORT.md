# Latency shape check

Ran 30 trials, 200 validators (f=66), 5 concurrent proposers per slot, over a geo-plausible 5-region delay matrix (same-region ~8ms, cross-region 60-220ms, +/-15ms jitter). Samples pool every validator's own observed latency across all trials, in ticks (treated as ms), measured from slot start (when proposers disseminate) to speculative/full finality.

## Distributions

| | avg | p50 | p90 | p99 |
|---|---|---|---|---|
| finalization (ms from slot start) | 577.8 | 579 | 610 | 612 |
| speculative finalization | 407.5 | 415 | 445 | 447 |

Transaction inclusion wait (not simulated — analytic, per the paper's own "half the block interval" estimate at a 100ms block interval): ~50ms.

## Round decomposition

Per validator, per finalized slot, the trace gives four ticks: (a) slot start / dissemination send, (b) the last Disseminate this validator actually accepted, (c) its own speculative (round-1-quorum) tick, (d) its own full-finality (round-2-quorum) tick. Averaged across all trials:

| segment | avg |
|---|---|
| D = b - a  (real observed dissemination delay) | 197.0ms |
| round-1 gap = c - b | 210.5ms |
| round-2 gap = d - c  (delta, a clean single voting round) | 170.3ms |

d - c is the cleanest available estimate of a single voting round (delta) in this implementation, because both endpoints are network-driven events rather than the fixed deadline. delta approx 170ms here, versus the paper's delta approx 52ms — expected, given this run's invented delay matrix goes up to 220ms cross-region against the paper's real Monad backbone. round-1 gap (c - b) is *not* a clean delta: round-1 votes fire on a fixed deadline timer (delta-parameter above the slot start), not on receipt of dissemination, so c - b also absorbs whatever slack is left in that fixed window after dissemination actually completed.

## Comparison to the paper (Section 8)

The paper reports, over Monad mainnet's real 200-validator geography with 5 proposers: 219ms average finalization, 167ms average speculative finality (speculative/final ratio ~0.76), and ~50ms average wait to enter a proposal at a 100ms block interval.

This run's speculative/final ratio, measured from slot start, is 0.71 (407.5ms / 577.8ms) — within a reasonable band of the paper's ~0.76. An earlier version of this experiment measured both latencies from the fixed per-slot deadline instead of from slot start, which produced a ratio of ~0.48: that anchor discarded the entire dissemination-delay component (D above) from both numbers, since the deadline is a configured buffer sized to absorb dissemination, not an event tied to it. Because D is nonzero and roughly comparable in size to a single voting round here, dropping it moved the ratio away from ~0.5 (pure delta-vs-2*delta) toward something further from the paper's D-dominated ~0.76. Anchoring to slot start instead restores D to both numbers and the ratio moves back into the paper's band. This was a measurement bug, not a protocol bug: the round count itself was always correct (one voting round to speculative, two to full finality — verified directly against the round-1/round-2 gaps above), only where the clock started was wrong.

## Honest caveats

- The 5-region delay matrix here is invented to be geo-plausible, not Monad's actual measured inter-validator latencies (which aren't published in a reproducible form). Absolute numbers are not expected to match the paper's; only the *shape* (speculative substantially faster than final, finalization on the order of a couple hundred ms at these delay magnitudes, ratio in the same neighborhood as the paper's) is being compared.
- This experiment measures only Chorus's own fast-path latency for a single slot; it does not run the full Conductor/pipeline scheduling loop, so it says nothing about steady-state throughput or the block-interval-driven inclusion wait (which is reported analytically above, not simulated).
- No fallback-path latency is included: FALLBACK_TIMEOUT is set high enough that, at these delay magnitudes, every trial's proposers reach fast-path quorum before it ever fires.

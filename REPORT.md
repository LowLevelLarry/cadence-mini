# REPORT — M6 latency shape check

Ran 30 trials, 200 validators (f=66), 5 concurrent proposers per slot, over a geo-plausible 5-region delay matrix (same-region ~8ms, cross-region 60-220ms, +/-15ms jitter). Samples pool every validator's own observed latency across all trials, in ticks (treated as ms).

## Distributions

| | avg | p50 | p90 | p99 |
|---|---|---|---|---|
| finalization (ticks past deadline) | 327.8 | 329 | 360 | 362 |
| speculative finalization | 157.5 | 165 | 195 | 197 |

Transaction inclusion wait (not simulated — analytic, per the paper's own "half the block interval" estimate at a 100ms block interval): ~50ms.

## Comparison to the paper (Section 8)

The paper reports, over Monad mainnet's real 200-validator geography with 5 proposers: 219ms average finalization, 167ms average speculative finality (speculative/final ratio ~0.76), and ~50ms average wait to enter a proposal at a 100ms block interval.

This run's speculative/final ratio is 0.48 (157.5ms / 327.8ms). That diverges more than expected from the paper's ~0.76 ratio — worth investigating if this experiment is extended.

The 2x-median-one-way-delay shape check (finalization ~= 2 * median one-way delay, since the fast path is D + 2*delta): this run's median finalization latency is 329ms.

## Honest caveats

- The 5-region delay matrix here is invented to be geo-plausible, not Monad's actual measured inter-validator latencies (which aren't published in a reproducible form) — see NOTES.md ambiguity #7. Absolute numbers are not expected to match the paper's; only the *shape* (speculative substantially faster than final, finalization on the order of a couple hundred ms at these delay magnitudes) is being compared.
- This experiment measures only Chorus's own fast-path latency for a single slot; it does not run the full Conductor/pipeline scheduling loop, so it says nothing about steady-state throughput or the block-interval-driven inclusion wait (which is reported analytically above, not simulated).
- No fallback-path latency is included: FALLBACK_TIMEOUT is set high enough that, at these delay magnitudes, every trial's proposers reach fast-path quorum before it ever fires.

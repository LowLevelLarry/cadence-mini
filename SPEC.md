# spec

Property -> test-name -> milestone ledger. One row per test currently in
`tests/properties/`. Keep this in sync with the actual file layout on disk — if a test gets
renamed or a file gets split, update the row here in the same commit.

| milestone | property | test |
|---|---|---|
| M1 — simulator core | same seed produces a byte-identical trace | `sim_determinism::same_seed_same_trace` |
| M1 | different seeds generally produce different traces | `sim_determinism::different_seeds_generally_differ` |
| M1 | a fixed delay model is deterministic too | `sim_determinism::fixed_delay_is_deterministic_too` |
| M1 | a configured link delay shows up exactly in the trace | `sim_delay::configured_delay_observed_in_trace` |
| M1 | no wall-clock/thread APIs anywhere in `src/` | `sim_no_wallclock::grep_gate_no_forbidden_apis` |
| M2 — single-slot Chorus | no two honest validators ever finalize conflicting blocks, even under adversarial delay variance | `chorus_safety::no_conflicting_finalization` |
| M2 | finalization actually happens in the sanity case (not vacuously passing safety) | `chorus_safety::sanity_finalization_actually_happens` |
| M2 | all-honest validators under synchrony finalize in exactly 3 rounds (disseminate, round-1 vote, fast vote) | `chorus_fast_path::fast_path_three_rounds` |
| M2 | finalization still completes with f byzantine-silent validators | `chorus_resilience::finalizes_at_f_silent_bound` |
| M2 | it stalls safely (no unsound finalization) beyond the f bound | `chorus_resilience::stalls_safely_beyond_bound` |
| M2 | every honest validator eventually finalizes (§3.1 termination) | `chorus_liveness::always_terminates` |
| M3 — extreme pipelining | many chorus instances are genuinely concurrent, not sequential, under long link delays | `pipeline_independence::five_concurrent_instances` |
| M3 | halving the block interval roughly doubles finalized-blocks-per-tick | `pipeline_decoupling::halving_interval_doubles_throughput` |
| M3 | outstanding instances never exceed the `2W - p` boundedness bound, even under injected instability | `pipeline_throttling::outstanding_never_exceeds_bound` |
| M4 — multiple concurrent proposers | a transaction submitted to exactly one correct proposer still gets included (censorship resistance) | `mcp_censorship_resistance::single_honest_proposer_tx_included` |
| M4 | no proposer's own dissemination causally follows another proposer's (hiding) | `mcp_hiding::no_causal_dependency_between_proposals` |
| M4 | fast-path round count with k proposers equals the single-proposer round count (no aggregation tax) | `mcp_no_aggregation_tax::round_count_independent_of_k` |
| M5 — speculative finality | speculative finality is never reverted in a run with no equivocation | `speculative_soundness::no_revert_without_equivocation` |
| M5 | equivocation triggers a correctly-justified revert | `speculative_revert::equivocation_triggers_correct_revert` |
| M5 | `justifying_proof` matches the proposer whose entry actually changed, not just any proof on hand | `speculative_revert::justifying_proof_matches_the_changed_proposer` |
| M5 — adversary suite | safety holds under an equivocating proposer | `adversary_suite::safety_holds_under_equivocator` |
| M5 | safety holds under a censoring proposer | `adversary_suite::safety_holds_under_censor` |
| M5 | safety holds with offline validators at the f bound | `adversary_suite::safety_holds_under_offline` |
| M5 | safety holds through a network partition-and-heal | `adversary_suite::safety_holds_under_partition_heal` |
| M6 — latency experiment | not a property test — `experiments/latency.rs` is a 200-validator, 5-proposer shape check against the paper's Section 8 numbers, writing `REPORT.md`. See NOTES.md entry 6 for why absolute numbers aren't expected to match. | n/a (`cargo run --release --bin latency`) |

Everything above lives in one binary (`tests/properties.rs`, which just declares
`#[path=...] mod` for each file under `tests/properties/`), so `cargo test --test properties`
runs the whole ledger in one shot.

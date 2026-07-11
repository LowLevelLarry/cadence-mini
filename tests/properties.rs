// single test binary that pulls in every gate's property tests from tests/properties/,
// so the file layout in SPEC.md matches what's actually on disk

#[path = "properties/sim_determinism.rs"]
mod sim_determinism;

#[path = "properties/sim_delay.rs"]
mod sim_delay;

#[path = "properties/sim_no_wallclock.rs"]
mod sim_no_wallclock;

#[path = "properties/chorus_common.rs"]
mod chorus_common;

#[path = "properties/chorus_safety.rs"]
mod chorus_safety;

#[path = "properties/chorus_fast_path.rs"]
mod chorus_fast_path;

#[path = "properties/chorus_resilience.rs"]
mod chorus_resilience;

#[path = "properties/chorus_liveness.rs"]
mod chorus_liveness;

#[path = "properties/pipeline_independence.rs"]
mod pipeline_independence;

#[path = "properties/pipeline_decoupling.rs"]
mod pipeline_decoupling;

#[path = "properties/pipeline_throttling.rs"]
mod pipeline_throttling;

#[path = "properties/mcp_censorship_resistance.rs"]
mod mcp_censorship_resistance;

#[path = "properties/mcp_hiding.rs"]
mod mcp_hiding;

#[path = "properties/mcp_no_aggregation_tax.rs"]
mod mcp_no_aggregation_tax;

#[path = "properties/speculative_soundness.rs"]
mod speculative_soundness;

#[path = "properties/speculative_revert.rs"]
mod speculative_revert;

#[path = "properties/adversary_suite.rs"]
mod adversary_suite;

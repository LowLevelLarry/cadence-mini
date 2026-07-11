// single test binary that pulls in every gate's property tests from tests/properties/,
// so the file layout in SPEC.md matches what's actually on disk

#[path = "properties/sim_determinism.rs"]
mod sim_determinism;

#[path = "properties/sim_delay.rs"]
mod sim_delay;

#[path = "properties/sim_no_wallclock.rs"]
mod sim_no_wallclock;

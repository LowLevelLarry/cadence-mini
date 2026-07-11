// §4.1/§4.4/§6.2: multiple concurrent proposers. Chorus already treats a slot's proposer set
// as a vector of independent entries (one certified inclusion/exclusion per proposer, voted
// on within the *same* two rounds regardless of k) — MCP is not a separate phase bolted onto
// single-leader consensus, it falls out of the meta-block structure itself. This module only
// adds what's genuinely MCP-specific: picking a slot's k proposers, and the deterministic
// merge from a finalized meta-block's recovered proposals into one block (§6.2), which lives
// in `types::merge_block` and is exercised here.

use crate::pipeline::ConductorConfig;
use crate::types::{ProposerId, Slot};

// deterministic, rotating window of k proposers per slot — every validator computes the same
// set locally, no coordination needed (the proposer *set* is public; only the proposals
// themselves are hidden until the deadline).
pub fn k_proposers_for_slot(cfg: &ConductorConfig, slot: Slot, k: usize) -> Vec<ProposerId> {
    let n = cfg.validators.len();
    let k = k.min(n);
    (0..k)
        .map(|i| cfg.validators[(slot as usize + i) % n])
        .collect()
}

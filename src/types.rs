use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub type ValidatorId = u64;
pub type ProposerId = ValidatorId;
pub type Slot = u64;

// stand-in for a Merkle root over an erasure-coded, encrypted proposal (§4.1). cadence-mini
// has no real crypto (Hard Rule #2), so a digest is just a deterministic hash of the payload
// — it plays exactly the role the paper's root plays: two proposals with the same content
// have the same digest, different content different digests, and nothing about it is secret.
pub type Digest = u64;

pub fn digest_of(payload: &[u64]) -> Digest {
    let mut h = DefaultHasher::new();
    payload.hash(&mut h);
    h.finish()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub slot: Slot,
    pub proposer: ProposerId,
    pub payload: Vec<u64>,
}

impl Proposal {
    pub fn digest(&self) -> Digest {
        digest_of(&self.payload)
    }
}

// one entry per proposer in a meta-block (§4.1, Figure 7): either a certified digest
// (the proposal is included) or bottom (correctly excluded).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Entry {
    Included(Digest),
    Excluded,
}

// the object Chorus actually agrees on: one entry per proposer of the slot.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MetaBlock {
    pub slot: Slot,
    pub entries: Vec<(ProposerId, Entry)>,
}

impl MetaBlock {
    // canonical form used for equality/dedup and as the fallback-agreement decision key —
    // proposer order is always ascending so two structurally-equal meta-blocks compare equal
    // regardless of the order entries were collected in
    pub fn canonical(mut self) -> Self {
        self.entries.sort_by_key(|(p, _)| *p);
        self
    }
}

// §4.4: merging a finalized meta-block's recovered proposals into the slot's block.
// deterministic: dedupe by (proposer, payload-content) isn't needed since Chorus recovery
// already guarantees a unique recovered payload per included proposer; the merge here is the
// MCP merge rule of §6.2 — concat included proposals in proposer order, dedupe transactions
// (a u64 "transaction id" stand-in) keeping first occurrence, preserve relative order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub slot: Slot,
    pub txs: Vec<u64>,
}

pub fn merge_block(slot: Slot, recovered: &[(ProposerId, Option<Vec<u64>>)]) -> Block {
    let mut ordered = recovered.to_vec();
    ordered.sort_by_key(|(p, _)| *p);
    let mut seen = std::collections::HashSet::new();
    let mut txs = Vec::new();
    for (_, payload) in ordered {
        if let Some(payload) = payload {
            for tx in payload {
                if seen.insert(tx) {
                    txs.push(tx);
                }
            }
        }
    }
    Block { slot, txs }
}

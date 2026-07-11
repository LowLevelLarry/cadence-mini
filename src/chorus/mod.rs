pub mod instance;
pub mod validator;

use crate::types::{Digest, MetaBlock, ProposerId, Slot, ValidatorId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RoundVote {
    Yes { digest: Digest, payload: Vec<u64> },
    No,
}

impl RoundVote {
    pub fn digest(&self) -> Option<Digest> {
        match self {
            RoundVote::Yes { digest, .. } => Some(*digest),
            RoundVote::No => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChorusMsg {
    // §4.2: a proposer's dissemination of its (plaintext, in cadence-mini) proposal.
    Disseminate {
        slot: Slot,
        proposer: ProposerId,
        payload: Vec<u64>,
    },
    // §4.2 round 1: one vote per proposer, cast at the deadline.
    Round1Vote {
        slot: Slot,
        proposer: ProposerId,
        vote: RoundVote,
    },
    // §4.2 round 2: a vote on the whole assembled fast meta-block.
    FastVote { slot: Slot, meta_block: MetaBlock },
    // §4.3: a validator that times out on the fast path bundles its no-fast-vote with its
    // per-proposer fallback votes into a single message (the paper sends these together).
    FallbackBundle {
        slot: Slot,
        votes: Vec<(ProposerId, RoundVote)>,
    },
    // §4.3: the meta-block (fast or fallback) a validator proposes to the fallback agreement.
    FallbackPropose { slot: Slot, meta_block: MetaBlock },
    // simplified leader-based fallback agreement (see NOTES.md ambiguity #2): the designated
    // leader's decision, and the echo round that certifies it.
    FallbackDecide { slot: Slot, meta_block: MetaBlock },
    FallbackEcho { slot: Slot, meta_block: MetaBlock },
}

impl ChorusMsg {
    pub fn slot(&self) -> Slot {
        match self {
            ChorusMsg::Disseminate { slot, .. }
            | ChorusMsg::Round1Vote { slot, .. }
            | ChorusMsg::FastVote { slot, .. }
            | ChorusMsg::FallbackBundle { slot, .. }
            | ChorusMsg::FallbackPropose { slot, .. }
            | ChorusMsg::FallbackDecide { slot, .. }
            | ChorusMsg::FallbackEcho { slot, .. } => *slot,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ProposerBehavior {
    Honest,
    Silent,
    // adversary/equivocator.rs (M5) builds on this: send `payload_a` to `split_a` and
    // `payload_b` to everyone else — a real proposer-level equivocation.
    Equivocate {
        payload_a: Vec<u64>,
        payload_b: Vec<u64>,
        split_a: Vec<ValidatorId>,
    },
    // adversary/censor.rs (M5): disseminate, but never a specific transaction id.
    Censor { payload: Vec<u64> },
}

#[derive(Debug, Clone)]
pub struct ChorusConfig {
    pub slot: Slot,
    pub deadline: crate::sim::Tick,
    pub delta: crate::sim::Tick,
    pub fallback_timeout: crate::sim::Tick,
    pub proposers: Vec<ProposerId>,
    pub validators: Vec<ValidatorId>,
    pub n: usize,
    pub f: usize,
}

// §4.3 / §4.5: proof that a proposer equivocated — two groups of >= f+1 validators each
// attesting (via round-1 yes votes) to a different digest for the same (slot, proposer).
// This is what licenses reverting a speculative finalization (NOTES.md §4, "equivocation
// proof").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquivocationProof {
    pub slot: Slot,
    pub proposer: ProposerId,
    pub digest_a: Digest,
    pub witnesses_a: Vec<ValidatorId>,
    pub digest_b: Digest,
    pub witnesses_b: Vec<ValidatorId>,
}

impl ChorusConfig {
    pub fn start_time(&self) -> crate::sim::Tick {
        self.deadline.saturating_sub(self.delta)
    }

    // deterministic per-slot fallback-agreement leader (NOTES.md ambiguity #2): rotates by
    // slot number over the full validator set so a single Byzantine validator can't
    // permanently wedge the fallback path across every slot.
    pub fn fallback_leader(&self) -> ValidatorId {
        let mut sorted = self.validators.clone();
        sorted.sort_unstable();
        sorted[(self.slot as usize) % sorted.len()]
    }
}

// a proposer that disseminates two conflicting proposals, sending `payload_a` to one half
// of the validator set and `payload_b` to the other. this is exactly the misbehavior §4.3
// condition (ii) and §4.5's safety-of-speculative-finalization proof are built around: it's
// the only way an honest validator ever ends up backing two different digests for the same
// (slot, proposer) with >= f+1 witnesses each.

use crate::chorus::ProposerBehavior;
use crate::types::ValidatorId;

pub fn equivocate(payload_a: Vec<u64>, payload_b: Vec<u64>, split_a: Vec<ValidatorId>) -> ProposerBehavior {
    ProposerBehavior::Equivocate { payload_a, payload_b, split_a }
}

// canonical split: sends payload_a to the first half of `validators` (by the given order)
// and payload_b to the rest.
pub fn half_split(validators: &[ValidatorId]) -> Vec<ValidatorId> {
    validators[..validators.len() / 2].to_vec()
}

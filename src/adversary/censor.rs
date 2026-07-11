// a proposer that disseminates normally (so it's never excluded on the fast path) but never
// includes a specific transaction — the thing short-term censorship resistance (Gate 4a)
// guarantees a *correct* proposer defeats: as long as one other proposer includes the
// transaction, this proposer's own choice not to can't keep it out of the finalized block.

use crate::chorus::ProposerBehavior;

pub fn censor(payload_without_target: Vec<u64>) -> ProposerBehavior {
    ProposerBehavior::Censor { payload: payload_without_target }
}

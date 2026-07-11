// §4.2/§4.5: speculative finality is confirmed after round 1 (one round earlier than full
// finality) and is only ever wrong — reverted in favor of whatever full finalization actually
// decides — when the slot's fallback path decided differently, which the paper's proof shows
// can only happen if some validator equivocated. This module gives that a structured, typed
// shape for tests instead of making them grep ChorusInstance's log strings.

use crate::chorus::instance::ChorusInstance;
use crate::chorus::EquivocationProof;
use crate::types::MetaBlock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeculativeOutcome {
    // no speculative view was ever formed for this slot at this validator (e.g. still
    // waiting on a round-1 quorum for some proposer)
    NoSpeculation,
    // speculated, not yet fully finalized
    Pending { speculative: MetaBlock },
    // full finalization agreed with the speculative view — the common, expected case
    Confirmed { meta_block: MetaBlock },
    // full finalization diverged from the speculative view — only sound if backed by an
    // equivocation proof (see `justified_by_equivocation` below)
    Reverted { speculative: MetaBlock, finalized: MetaBlock },
}

pub fn outcome(instance: &ChorusInstance) -> SpeculativeOutcome {
    match (&instance.speculative, &instance.finalized) {
        (Some(spec), Some(fin)) if spec.meta_block == fin.meta_block => {
            SpeculativeOutcome::Confirmed { meta_block: fin.meta_block.clone() }
        }
        (Some(spec), Some(fin)) => SpeculativeOutcome::Reverted {
            speculative: spec.meta_block.clone(),
            finalized: fin.meta_block.clone(),
        },
        (Some(spec), None) => SpeculativeOutcome::Pending { speculative: spec.meta_block.clone() },
        (None, _) => SpeculativeOutcome::NoSpeculation,
    }
}

// a revert is only sound if there's a proof — two groups of >= f+1 validators each attesting
// (via round-1 votes) to a different digest for the same (slot, proposer) — implicating the
// specific proposer whose entry actually changed between the speculative and final view.
pub fn justifying_proof<'a>(
    proofs: &'a [EquivocationProof],
    speculative: &MetaBlock,
    finalized: &MetaBlock,
) -> Option<&'a EquivocationProof> {
    let changed_proposers: Vec<_> = speculative
        .entries
        .iter()
        .zip(&finalized.entries)
        .filter(|((_, se), (_, fe))| se != fe)
        .map(|((p, _), _)| *p)
        .collect();
    proofs.iter().find(|proof| changed_proposers.contains(&proof.proposer))
}

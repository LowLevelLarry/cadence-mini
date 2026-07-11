// a byzantine-silent validator: never sends a single message, as if crashed. this is the
// `muted` flag ChorusInstance/PipelineValidator already carry — this module just names the
// adversary and picks how many validators (up to f, the paper's resilience bound) go dark.

use crate::types::ValidatorId;

pub fn offline_set(validators: &[ValidatorId], f: usize) -> Vec<ValidatorId> {
    validators.iter().copied().take(f).collect()
}

// partition-and-heal: the network is asynchronous (large, adversarially-varying delay) until
// a fixed tick, then behaves per the partial-synchrony model (bounded delay) from then on —
// literally GST, made concrete for the simulator (see sim::PartitionHeal).

use crate::sim::{PartitionHeal, Tick};

pub fn partition_then_heal(heals_at: Tick, asynchronous_delay: Tick, synchronous_delay: Tick) -> PartitionHeal {
    PartitionHeal { heals_at, asynchronous_delay, synchronous_delay }
}

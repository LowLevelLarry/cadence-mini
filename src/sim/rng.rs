use rand::rngs::StdRng;
use rand::SeedableRng;

// one deterministic stream per node, derived from the run seed so a single seed still
// reproduces byte-identical per-node randomness regardless of node count or id spacing
pub fn node_rng(run_seed: u64, node_id: u64) -> StdRng {
    StdRng::seed_from_u64(splitmix64(run_seed ^ splitmix64(node_id.wrapping_add(0x9E37_79B9_7F4A_7C15))))
}

pub fn link_rng(run_seed: u64, from: u64, to: u64) -> StdRng {
    let mixed = splitmix64(run_seed ^ splitmix64(from.wrapping_mul(0x100_0000_01B3).wrapping_add(to)));
    StdRng::seed_from_u64(mixed)
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

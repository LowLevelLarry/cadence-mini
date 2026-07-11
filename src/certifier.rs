use crate::types::ValidatorId;
use std::collections::HashMap;
use std::hash::Hash;

// Hard Rule #2: crypto is a trait, mocked forever. A Certificate is just the set of
// (validator, vote) pairs that back a value plus the threshold it met — no signatures,
// no aggregation, nothing the protocol could tell apart from "real" crypto by inspecting it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Certificate<V> {
    pub value: V,
    pub signers: Vec<ValidatorId>,
}

pub trait Certifier {
    fn n(&self) -> usize;
    fn f(&self) -> usize;

    fn quorum_2f_plus_1(&self) -> usize {
        2 * self.f() + 1
    }

    fn quorum_f_plus_1(&self) -> usize {
        self.f() + 1
    }

    // looks for a value with at least `threshold` distinct signers among `votes`; returns the
    // first such certificate found (by value's natural iteration order — callers that need a
    // specific one, e.g. "the root with 2f+1 yes votes", filter votes down to a single value's
    // signers before calling this)
    fn certify<V: Eq + Hash + Clone>(
        &self,
        votes: &HashMap<ValidatorId, V>,
        threshold: usize,
    ) -> Option<Certificate<V>> {
        let mut by_value: HashMap<V, Vec<ValidatorId>> = HashMap::new();
        for (&signer, value) in votes {
            by_value.entry(value.clone()).or_default().push(signer);
        }
        for (value, mut signers) in by_value {
            if signers.len() >= threshold {
                signers.sort_unstable();
                return Some(Certificate { value, signers });
            }
        }
        None
    }
}

pub struct ThresholdCertifier {
    pub n: usize,
    pub f: usize,
}

impl Certifier for ThresholdCertifier {
    fn n(&self) -> usize {
        self.n
    }
    fn f(&self) -> usize {
        self.f
    }
}

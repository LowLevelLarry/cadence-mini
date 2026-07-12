// the actual Chorus slot-consensus logic (§4), extracted from any particular Node/engine
// wiring so it can run either as its own single-slot Node (chorus/validator.rs) or as one of
// many concurrently-running instances inside a single pipelined validator (pipeline.rs). An
// instance never talks to another instance directly and carries no reference to any other
// slot — that separation is what extreme pipelining rests on.
//
// timers this instance schedules are always named "{slot}#name" so a caller juggling many
// instances for one validator can route a fired timer to the right one without this code
// needing to know it's being multiplexed.

use super::{ChorusConfig, ChorusMsg, EquivocationProof, ProposerBehavior, RoundVote};
use crate::certifier::{Certifier, ThresholdCertifier};
use crate::sim::{Ctx, Tick};
use crate::types::{merge_block, Block, Digest, Entry, MetaBlock, ProposerId, Slot, ValidatorId};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedSlot {
    pub meta_block: MetaBlock,
    pub via_fallback: bool,
    // round (from slot open) at which this validator observed finality
    pub round: u32,
}

pub fn timer_name(slot: Slot, name: &str) -> String {
    format!("{slot}#{name}")
}

pub fn timer_slot(timer: &str) -> Option<Slot> {
    timer.split('#').next()?.parse().ok()
}

pub struct ChorusInstance {
    pub id: ValidatorId,
    pub config: ChorusConfig,
    pub behavior: HashMap<ProposerId, ProposerBehavior>,
    // a byzantine-silent validator: never sends anything, as if crashed/offline. distinct
    // from ProposerBehavior::Silent, which mutes only a proposer's dissemination while it
    // still votes normally as a validator.
    pub muted: bool,
    certifier: ThresholdCertifier,

    // proposals this validator has directly seen disseminated (proposer -> (digest, payload))
    received: HashMap<ProposerId, (u64, Vec<u64>)>,

    round1_votes: HashMap<ProposerId, HashMap<ValidatorId, RoundVote>>,
    cast_round1: bool,

    fast_meta_block: Option<MetaBlock>,
    pub speculative: Option<FinalizedSlot>,
    fast_votes: HashMap<ValidatorId, MetaBlock>,
    cast_fast_vote: bool,

    fallback_timer_fired: bool,
    sent_fallback_bundle: bool,
    second_round_senders: HashSet<ValidatorId>,
    fallback_votes: HashMap<ProposerId, HashMap<ValidatorId, RoundVote>>,
    sent_fallback_propose: bool,

    fallback_proposals: HashMap<ValidatorId, MetaBlock>,
    sent_fallback_decide: bool,
    echoed: Option<MetaBlock>,
    fallback_echoes: HashMap<ValidatorId, MetaBlock>,

    pub finalized: Option<FinalizedSlot>,
    pub equivocation_proofs: Vec<EquivocationProof>,
    round1_tick: Option<Tick>,
    round2_tick: Option<Tick>,
    // wall-clock (sim tick) of the moment this validator actually reached each milestone —
    // used by experiments/latency.rs (M6) to measure latency, distinct from round1_tick/
    // round2_tick above which only record when *this validator cast its own* vote.
    pub speculative_tick: Option<Tick>,
    pub finalized_tick: Option<Tick>,
    // tick of the last Disseminate this validator actually accepted before the deadline —
    // used by experiments/latency.rs to decompose measured latency into a dissemination
    // component and a voting component instead of lumping both into "past the deadline".
    pub last_dissemination_tick: Option<Tick>,
}

impl ChorusInstance {
    pub fn new(id: ValidatorId, config: ChorusConfig, behavior: HashMap<ProposerId, ProposerBehavior>) -> Self {
        Self::new_with_muted(id, config, behavior, false)
    }

    pub fn new_with_muted(
        id: ValidatorId,
        config: ChorusConfig,
        behavior: HashMap<ProposerId, ProposerBehavior>,
        muted: bool,
    ) -> Self {
        let certifier = ThresholdCertifier { n: config.n, f: config.f };
        Self {
            id,
            config,
            behavior,
            muted,
            certifier,
            received: HashMap::new(),
            round1_votes: HashMap::new(),
            cast_round1: false,
            fast_meta_block: None,
            speculative: None,
            fast_votes: HashMap::new(),
            cast_fast_vote: false,
            fallback_timer_fired: false,
            sent_fallback_bundle: false,
            second_round_senders: HashSet::new(),
            fallback_votes: HashMap::new(),
            sent_fallback_propose: false,
            fallback_proposals: HashMap::new(),
            sent_fallback_decide: false,
            echoed: None,
            fallback_echoes: HashMap::new(),
            finalized: None,
            equivocation_proofs: Vec::new(),
            round1_tick: None,
            round2_tick: None,
            speculative_tick: None,
            finalized_tick: None,
            last_dissemination_tick: None,
        }
    }

    pub fn slot(&self) -> Slot {
        self.config.slot
    }

    // §4.4: from a finalized meta-block to the slot's block — recover each included
    // proposer's payload from whatever this validator holds (direct dissemination, a round-1
    // yes vote, or a fallback-yes vote all carry the payload), then apply the MCP merge rule.
    // Returns None until this validator has actually finalized the slot.
    pub fn recovered_block(&self) -> Option<Block> {
        let f = self.finalized.as_ref()?;
        let recovered: Vec<(ProposerId, Option<Vec<u64>>)> = f
            .meta_block
            .entries
            .iter()
            .map(|(proposer, entry)| {
                let payload = match entry {
                    Entry::Included(digest) => self.recover_payload(*proposer, *digest),
                    Entry::Excluded => None,
                };
                (*proposer, payload)
            })
            .collect();
        Some(merge_block(f.meta_block.slot, &recovered))
    }

    fn recover_payload(&self, proposer: ProposerId, digest: Digest) -> Option<Vec<u64>> {
        if let Some((d, payload)) = self.received.get(&proposer)
            && *d == digest
        {
            return Some(payload.clone());
        }
        for votes in [self.round1_votes.get(&proposer), self.fallback_votes.get(&proposer)]
            .into_iter()
            .flatten()
        {
            for vote in votes.values() {
                if let RoundVote::Yes { digest: d, payload } = vote
                    && *d == digest
                {
                    return Some(payload.clone());
                }
            }
        }
        None
    }

    fn is_proposer(&self) -> bool {
        self.config.proposers.contains(&self.id)
    }

    fn disseminate(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        let behavior = self
            .behavior
            .get(&self.id)
            .cloned()
            .unwrap_or(ProposerBehavior::Honest);
        match behavior {
            ProposerBehavior::Silent => {}
            ProposerBehavior::Honest => {
                let payload = vec![self.id * 1000 + self.config.slot];
                self.received.insert(self.id, (crate::types::digest_of(&payload), payload.clone()));
                ctx.broadcast(
                    &self.config.validators,
                    ChorusMsg::Disseminate {
                        slot: self.config.slot,
                        proposer: self.id,
                        payload,
                    },
                );
            }
            ProposerBehavior::Censor { payload } => {
                self.received.insert(self.id, (crate::types::digest_of(&payload), payload.clone()));
                ctx.broadcast(
                    &self.config.validators,
                    ChorusMsg::Disseminate {
                        slot: self.config.slot,
                        proposer: self.id,
                        payload,
                    },
                );
            }
            ProposerBehavior::Equivocate { payload_a, payload_b, split_a } => {
                for &v in &self.config.validators {
                    let payload = if split_a.contains(&v) { payload_a.clone() } else { payload_b.clone() };
                    if v == self.id {
                        self.received.insert(self.id, (crate::types::digest_of(&payload), payload.clone()));
                    }
                    ctx.send(
                        v,
                        ChorusMsg::Disseminate {
                            slot: self.config.slot,
                            proposer: self.id,
                            payload,
                        },
                    );
                }
            }
        }
    }

    fn cast_round1_votes(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        if self.cast_round1 {
            return;
        }
        self.cast_round1 = true;
        self.round1_tick = Some(ctx.tick);
        for &proposer in self.config.proposers.clone().iter() {
            let vote = match self.received.get(&proposer) {
                Some((digest, payload)) => RoundVote::Yes { digest: *digest, payload: payload.clone() },
                None => RoundVote::No,
            };
            self.round1_votes
                .entry(proposer)
                .or_default()
                .insert(self.id, vote.clone());
            ctx.broadcast(
                &self.config.validators,
                ChorusMsg::Round1Vote { slot: self.config.slot, proposer, vote },
            );
        }
        self.try_build_fast_meta_block(ctx);
        self.maybe_enter_fallback(ctx);
    }

    fn try_build_fast_meta_block(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        if self.fast_meta_block.is_some() {
            return;
        }
        let mut entries = Vec::new();
        for &proposer in &self.config.proposers {
            let votes = match self.round1_votes.get(&proposer) {
                Some(v) => v,
                None => return,
            };
            let cert = self.certifier.certify(votes, self.certifier.quorum_2f_plus_1());
            match cert {
                Some(c) => match c.value {
                    RoundVote::Yes { digest, .. } => entries.push((proposer, Entry::Included(digest))),
                    RoundVote::No => entries.push((proposer, Entry::Excluded)),
                },
                None => return, // no quorum yet for this proposer
            }
        }
        let meta_block = MetaBlock { slot: self.config.slot, entries }.canonical();
        self.fast_meta_block = Some(meta_block.clone());

        // §4.2: speculative finality, one round earlier than full finality
        if self.speculative.is_none() {
            self.speculative = Some(FinalizedSlot {
                meta_block: meta_block.clone(),
                via_fallback: false,
                round: 1,
            });
            self.speculative_tick = Some(ctx.tick);
            ctx.log(format!("slot {} speculatively finalized by validator {}", self.config.slot, self.id));
        }

        if !self.cast_fast_vote {
            self.cast_fast_vote = true;
            self.round2_tick = Some(ctx.tick);
            self.fast_votes.insert(self.id, meta_block.clone());
            ctx.broadcast(
                &self.config.validators,
                ChorusMsg::FastVote { slot: self.config.slot, meta_block },
            );
            self.try_finalize_fast(ctx);
        }
    }

    fn try_finalize_fast(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        if self.finalized.is_some() {
            return;
        }
        if let Some(cert) = self.certifier.certify(&self.fast_votes, self.certifier.quorum_2f_plus_1()) {
            self.finalize(ctx, cert.value, false, 2);
        }
    }

    fn finalize(&mut self, ctx: &mut Ctx<ChorusMsg>, meta_block: MetaBlock, via_fallback: bool, round: u32) {
        if self.finalized.is_some() {
            return;
        }
        // detect a revert relative to our own speculative view
        if let Some(spec) = &self.speculative
            && spec.meta_block != meta_block
        {
            ctx.log(format!(
                "slot {} REVERT at validator {}: speculative {:?} != final {:?}",
                self.config.slot, self.id, spec.meta_block, meta_block
            ));
        }
        ctx.log(format!(
            "slot {} finalized by validator {} via_fallback={via_fallback}",
            self.config.slot, self.id
        ));
        self.finalized = Some(FinalizedSlot { meta_block, via_fallback, round });
        self.finalized_tick = Some(ctx.tick);
    }

    fn maybe_enter_fallback(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        if self.fast_meta_block.is_some() || self.sent_fallback_bundle {
            return;
        }
        let distinct_voters: usize = self
            .round1_votes
            .values()
            .flat_map(|m| m.keys().copied())
            .collect::<HashSet<_>>()
            .len();
        if self.fallback_timer_fired && distinct_voters >= self.certifier.quorum_2f_plus_1() {
            self.enter_fallback(ctx);
        }
    }

    fn enter_fallback(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        if self.sent_fallback_bundle || self.fast_meta_block.is_some() {
            return;
        }
        self.sent_fallback_bundle = true;
        let mut votes = Vec::new();
        for &proposer in self.config.proposers.clone().iter() {
            let vote = self.compute_fallback_vote(proposer);
            self.fallback_votes.entry(proposer).or_default().insert(self.id, vote.clone());
            votes.push((proposer, vote));
        }
        self.second_round_senders.insert(self.id);
        ctx.broadcast(
            &self.config.validators,
            ChorusMsg::FallbackBundle { slot: self.config.slot, votes },
        );
        self.maybe_propose_to_fallback_agreement(ctx);
    }

    fn compute_fallback_vote(&mut self, proposer: ProposerId) -> RoundVote {
        let empty = HashMap::new();
        let votes = self.round1_votes.get(&proposer).unwrap_or(&empty);

        // group round-1 yes votes by digest first, so a tie between two digests that both
        // clear the f+1 threshold is caught as an equivocation rather than resolved by
        // whichever HashMap<Digest, _> happens to iterate first — HashMap's default hasher is
        // randomized per instance, so picking "the first one that clears threshold" would be
        // nondeterministic across validators (and could itself break safety, since two
        // validators could each accept a different one of the two conflicting digests).
        let mut by_digest: HashMap<u64, Vec<ValidatorId>> = HashMap::new();
        for (&voter, v) in votes.iter() {
            if let RoundVote::Yes { digest, .. } = v {
                by_digest.entry(*digest).or_default().push(voter);
            }
        }
        let mut strong: Vec<(u64, Vec<ValidatorId>)> = by_digest
            .into_iter()
            .filter(|(_, w)| w.len() >= self.certifier.quorum_f_plus_1())
            .collect();
        strong.sort_by_key(|(digest, _)| *digest); // canonical, not insertion/hash order

        if strong.len() >= 2 {
            let (da, wa) = strong[0].clone();
            let (db, wb) = strong[1].clone();
            self.equivocation_proofs.push(EquivocationProof {
                slot: self.config.slot,
                proposer,
                digest_a: da,
                witnesses_a: wa,
                digest_b: db,
                witnesses_b: wb,
            });
            return RoundVote::No;
        }

        if let Some((digest, witnesses)) = strong.into_iter().next() {
            let payload = witnesses.iter().find_map(|w| match votes.get(w) {
                Some(RoundVote::Yes { digest: d, payload }) if *d == digest => Some(payload.clone()),
                _ => None,
            });
            if let Some(payload) = payload {
                return RoundVote::Yes { digest, payload };
            }
        }
        RoundVote::No
    }

    fn maybe_propose_to_fallback_agreement(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        if self.sent_fallback_propose {
            return;
        }
        if self.second_round_senders.len() < self.certifier.quorum_2f_plus_1() {
            return;
        }
        self.sent_fallback_propose = true;
        let candidate = if let Some(mb) = self.fast_votes.values().next().cloned() {
            mb
        } else {
            let mut entries = Vec::new();
            for &proposer in &self.config.proposers {
                let entry = match self
                    .fallback_votes
                    .get(&proposer)
                    .and_then(|votes| self.certifier.certify(votes, self.certifier.quorum_f_plus_1()))
                {
                    Some(cert) => match cert.value {
                        RoundVote::Yes { digest, .. } => Entry::Included(digest),
                        RoundVote::No => Entry::Excluded,
                    },
                    None => Entry::Excluded,
                };
                entries.push((proposer, entry));
            }
            MetaBlock { slot: self.config.slot, entries }.canonical()
        };
        ctx.broadcast(
            &self.config.validators,
            ChorusMsg::FallbackPropose { slot: self.config.slot, meta_block: candidate },
        );
    }

    fn maybe_lead_fallback_agreement(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        if self.id != self.config.fallback_leader() || self.sent_fallback_decide {
            return;
        }
        if self.fallback_proposals.len() < self.certifier.quorum_2f_plus_1() {
            return;
        }
        self.sent_fallback_decide = true;
        // deterministic pick: highest-multiplicity value, canonical tie-break. this
        // leader-echo agreement is a simulation-only stand-in for the paper's black-box
        // fallback agreement (§4.3), not a general async BFT/ACS protocol.
        let mut counts: HashMap<MetaBlock, usize> = HashMap::new();
        for mb in self.fallback_proposals.values() {
            *counts.entry(mb.clone()).or_default() += 1;
        }
        let decision = counts
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then_with(|| format!("{:?}", b.0).cmp(&format!("{:?}", a.0))))
            .map(|(mb, _)| mb)
            .expect("at least one proposal received");
        ctx.broadcast(
            &self.config.validators,
            ChorusMsg::FallbackDecide { slot: self.config.slot, meta_block: decision },
        );
    }

    fn on_fallback_decide(&mut self, ctx: &mut Ctx<ChorusMsg>, from: ValidatorId, meta_block: MetaBlock) {
        if from != self.config.fallback_leader() || self.echoed.is_some() {
            return;
        }
        self.echoed = Some(meta_block.clone());
        self.fallback_echoes.insert(self.id, meta_block.clone());
        ctx.broadcast(
            &self.config.validators,
            ChorusMsg::FallbackEcho { slot: self.config.slot, meta_block },
        );
        self.try_finalize_fallback(ctx);
    }

    fn try_finalize_fallback(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        if self.finalized.is_some() {
            return;
        }
        if let Some(cert) = self.certifier.certify(&self.fallback_echoes, self.certifier.quorum_2f_plus_1()) {
            self.finalize(ctx, cert.value, true, 3);
        }
    }

    pub fn on_start(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        if self.muted {
            return;
        }
        let slot = self.config.slot;
        if self.is_proposer() {
            let start = self.config.start_time();
            if ctx.tick >= start {
                self.disseminate(ctx);
            } else {
                ctx.set_timer(start - ctx.tick, timer_name(slot, "disseminate"));
            }
        }
        let deadline = self.config.deadline;
        if ctx.tick >= deadline {
            self.cast_round1_votes(ctx);
        } else {
            ctx.set_timer(deadline - ctx.tick, timer_name(slot, "deadline"));
        }
        let timeout_at = deadline + self.config.fallback_timeout;
        if timeout_at > ctx.tick {
            ctx.set_timer(timeout_at - ctx.tick, timer_name(slot, "fallback_timeout"));
        }
    }

    pub fn on_message(&mut self, ctx: &mut Ctx<ChorusMsg>, from: ValidatorId, msg: ChorusMsg) {
        if self.muted {
            return;
        }
        match msg {
            ChorusMsg::Disseminate { slot, proposer, payload } => {
                if slot != self.config.slot || proposer != from {
                    return;
                }
                if ctx.tick > self.config.deadline {
                    return; // §4.2: dissemination after the deadline never counts
                }
                self.last_dissemination_tick = Some(self.last_dissemination_tick.map_or(ctx.tick, |t| t.max(ctx.tick)));
                self.received.entry(proposer).or_insert_with(|| (crate::types::digest_of(&payload), payload));
            }
            ChorusMsg::Round1Vote { slot, proposer, vote } => {
                if slot != self.config.slot {
                    return;
                }
                self.round1_votes.entry(proposer).or_default().insert(from, vote);
                self.try_build_fast_meta_block(ctx);
                self.maybe_enter_fallback(ctx);
            }
            ChorusMsg::FastVote { slot, meta_block } => {
                if slot != self.config.slot {
                    return;
                }
                self.fast_votes.insert(from, meta_block);
                self.second_round_senders.insert(from);
                self.try_finalize_fast(ctx);
                self.maybe_propose_to_fallback_agreement(ctx);
            }
            ChorusMsg::FallbackBundle { slot, votes } => {
                if slot != self.config.slot {
                    return;
                }
                self.second_round_senders.insert(from);
                for (proposer, vote) in votes {
                    self.fallback_votes.entry(proposer).or_default().insert(from, vote);
                }
                self.maybe_propose_to_fallback_agreement(ctx);
            }
            ChorusMsg::FallbackPropose { slot, meta_block } => {
                if slot != self.config.slot {
                    return;
                }
                self.fallback_proposals.insert(from, meta_block);
                self.maybe_lead_fallback_agreement(ctx);
            }
            ChorusMsg::FallbackDecide { slot, meta_block } => {
                if slot != self.config.slot {
                    return;
                }
                self.on_fallback_decide(ctx, from, meta_block);
            }
            ChorusMsg::FallbackEcho { slot, meta_block } => {
                if slot != self.config.slot {
                    return;
                }
                self.fallback_echoes.insert(from, meta_block);
                self.try_finalize_fallback(ctx);
            }
        }
    }

    pub fn on_timer(&mut self, ctx: &mut Ctx<ChorusMsg>, timer: &str) {
        if self.muted {
            return;
        }
        let slot = self.config.slot;
        if timer == timer_name(slot, "disseminate") {
            self.disseminate(ctx);
        } else if timer == timer_name(slot, "deadline") {
            self.cast_round1_votes(ctx);
        } else if timer == timer_name(slot, "fallback_timeout") {
            self.fallback_timer_fired = true;
            self.maybe_enter_fallback(ctx);
        }
    }
}

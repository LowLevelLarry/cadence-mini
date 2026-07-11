// §3 / §5: the extreme-pipelining framework, instantiated with Chorus as slot consensus and
// a simplified Conductor as orchestrator (NOTES.md §4, ambiguity #5). One PipelineValidator
// Node runs many ChorusInstances concurrently, one per open slot; nothing about slot s is
// required for slot s+1 to open — the only cross-slot coupling is the orchestrator's window
// throttle, which is a purely local (per-validator) bound, matching the paper's boundedness
// guarantee ("each honest validator has only a bounded number of slots underway").

use crate::chorus::instance::{timer_slot, ChorusInstance};
use crate::chorus::{ChorusConfig, ChorusMsg, ProposerBehavior};
use crate::sim::{Ctx, Node, Tick};
use crate::types::{ProposerId, Slot, ValidatorId};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ConductorConfig {
    pub n: usize,
    pub f: usize,
    pub delta: Tick,
    pub fallback_timeout: Tick,
    pub tau: Tick,      // block interval
    pub window: usize,  // W: slots per window
    pub threshold: usize, // p: how many of window w must complete before w+1 can open
    pub validators: Vec<ValidatorId>,
}

impl ConductorConfig {
    fn window_slots(&self, window: usize) -> std::ops::RangeInclusive<Slot> {
        let start = ((window - 1) * self.window) as u64 + 1;
        let end = start + self.window as u64 - 1;
        start..=end
    }
}

pub struct PipelineValidator {
    pub id: ValidatorId,
    cfg: ConductorConfig,
    proposer_for_slot: fn(&ConductorConfig, Slot) -> Vec<ProposerId>,
    behavior_for: fn(ValidatorId) -> HashMap<ProposerId, ProposerBehavior>,
    muted: HashSet<ValidatorId>,

    instances: HashMap<Slot, ChorusInstance>,
    completed: HashSet<Slot>,
    opened_windows: HashSet<usize>,
    window_first_deadline: HashMap<usize, Tick>,

    pub finalize_log: Vec<(Slot, Tick)>, // (slot, tick it was observed finalized locally)
    pub max_outstanding: usize,
}

impl PipelineValidator {
    pub fn new(
        id: ValidatorId,
        cfg: ConductorConfig,
        proposer_for_slot: fn(&ConductorConfig, Slot) -> Vec<ProposerId>,
        behavior_for: fn(ValidatorId) -> HashMap<ProposerId, ProposerBehavior>,
        muted: HashSet<ValidatorId>,
    ) -> Self {
        Self {
            id,
            cfg,
            proposer_for_slot,
            behavior_for,
            muted,
            instances: HashMap::new(),
            completed: HashSet::new(),
            opened_windows: HashSet::new(),
            window_first_deadline: HashMap::new(),
            finalize_log: Vec::new(),
            max_outstanding: 0,
        }
    }

    pub fn outstanding_now(&self) -> usize {
        self.instances.values().filter(|i| i.finalized.is_none()).count()
    }

    fn spawn_instance(&mut self, ctx: &mut Ctx<ChorusMsg>, slot: Slot, deadline: Tick) {
        if self.instances.contains_key(&slot) {
            return;
        }
        let proposers = (self.proposer_for_slot)(&self.cfg, slot);
        let config = ChorusConfig {
            slot,
            deadline,
            delta: self.cfg.delta,
            fallback_timeout: self.cfg.fallback_timeout,
            proposers,
            validators: self.cfg.validators.clone(),
            n: self.cfg.n,
            f: self.cfg.f,
        };
        let muted = self.muted.contains(&self.id);
        let mut instance =
            ChorusInstance::new_with_muted(self.id, config, (self.behavior_for)(self.id), muted);
        instance.on_start(ctx);
        self.instances.insert(slot, instance);
        self.max_outstanding = self.max_outstanding.max(self.outstanding_now());
    }

    fn open_window(&mut self, ctx: &mut Ctx<ChorusMsg>, window: usize, first_deadline: Tick) {
        if self.opened_windows.contains(&window) {
            return;
        }
        self.opened_windows.insert(window);
        self.window_first_deadline.insert(window, first_deadline);
        for slot in self.cfg.window_slots(window) {
            let offset = slot - *self.cfg.window_slots(window).start();
            self.spawn_instance(ctx, slot, first_deadline + offset * self.cfg.tau);
        }
    }

    fn window_fully_complete(&self, window: usize) -> bool {
        self.cfg.window_slots(window).all(|s| self.completed.contains(&s))
    }

    fn window_first_p_complete(&self, window: usize) -> bool {
        self.cfg
            .window_slots(window)
            .take(self.cfg.threshold)
            .all(|s| self.completed.contains(&s))
    }

    // §5.1: try to open the next window once its prerequisites are met. Called after every
    // newly-observed local completion, from any currently-open window (a completion in an
    // earlier window can be what finally unblocks things after a straggler catches up).
    fn maybe_advance(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        loop {
            let Some(&current_max_open) = self.opened_windows.iter().max() else { return };
            let next = current_max_open + 1;
            if self.opened_windows.contains(&next) {
                return;
            }
            let earlier_complete = (1..current_max_open).all(|w| self.window_fully_complete(w));
            if !(earlier_complete && self.window_first_p_complete(current_max_open)) {
                return;
            }
            let last_slot_deadline = *self
                .window_first_deadline
                .get(&current_max_open)
                .expect("open window has a first deadline")
                + (self.cfg.window as u64 - 1) * self.cfg.tau;
            // §5.1 Figure 11: healthy (unbroken cadence) vs lagging (jump to now) cases
            let next_first_deadline = if ctx.tick < last_slot_deadline {
                last_slot_deadline + self.cfg.tau
            } else {
                ctx.tick
            };
            self.open_window(ctx, next, next_first_deadline);
        }
    }

    fn on_possible_completion(&mut self, ctx: &mut Ctx<ChorusMsg>, slot: Slot) {
        if self.completed.contains(&slot) {
            return;
        }
        if let Some(instance) = self.instances.get(&slot)
            && instance.finalized.is_some()
        {
            self.completed.insert(slot);
            self.finalize_log.push((slot, ctx.tick));
            self.maybe_advance(ctx);
        }
    }
}

impl Node for PipelineValidator {
    type Message = ChorusMsg;

    fn id(&self) -> ValidatorId {
        self.id
    }

    fn on_start(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        // §5.1: window 1 opens at genesis with slot 1's deadline at Delta
        self.open_window(ctx, 1, self.cfg.delta);
        self.maybe_advance(ctx);
    }

    fn on_message(&mut self, ctx: &mut Ctx<ChorusMsg>, from: ValidatorId, msg: ChorusMsg) {
        let slot = msg.slot();
        if let Some(instance) = self.instances.get_mut(&slot) {
            instance.on_message(ctx, from, msg);
            self.on_possible_completion(ctx, slot);
        }
        // messages for slots not yet spawned (e.g. late arrivals about to-be-opened windows)
        // are simply dropped: a well-behaved peer only sends slot-s messages once it has
        // itself opened slot s, and slots open in the same order everywhere.
    }

    fn on_timer(&mut self, ctx: &mut Ctx<ChorusMsg>, timer: String) {
        let Some(slot) = timer_slot(&timer) else { return };
        if let Some(instance) = self.instances.get_mut(&slot) {
            instance.on_timer(ctx, &timer);
            self.on_possible_completion(ctx, slot);
        }
    }
}

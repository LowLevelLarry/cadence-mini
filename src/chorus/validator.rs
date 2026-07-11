// thin single-slot Node wrapper around ChorusInstance, used directly by the M2 gate tests
// (one slot, no pipelining). pipeline.rs (M3+) drives many ChorusInstances concurrently
// inside a single Node instead of using this wrapper.

use super::instance::{timer_slot, ChorusInstance};
use super::{ChorusConfig, ChorusMsg, ProposerBehavior};
use crate::sim::{Ctx, Node};
use crate::types::{ProposerId, ValidatorId};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

pub struct ChorusValidator {
    inner: ChorusInstance,
}

impl ChorusValidator {
    pub fn new(id: ValidatorId, config: ChorusConfig, behavior: HashMap<ProposerId, ProposerBehavior>) -> Self {
        Self { inner: ChorusInstance::new(id, config, behavior) }
    }

    pub fn new_with_muted(
        id: ValidatorId,
        config: ChorusConfig,
        behavior: HashMap<ProposerId, ProposerBehavior>,
        muted: bool,
    ) -> Self {
        Self { inner: ChorusInstance::new_with_muted(id, config, behavior, muted) }
    }
}

// exposes ChorusInstance's public fields/methods (id, config, finalized, speculative, ...)
// directly on ChorusValidator so existing test call sites don't need to change.
impl Deref for ChorusValidator {
    type Target = ChorusInstance;
    fn deref(&self) -> &ChorusInstance {
        &self.inner
    }
}

impl DerefMut for ChorusValidator {
    fn deref_mut(&mut self) -> &mut ChorusInstance {
        &mut self.inner
    }
}

impl Node for ChorusValidator {
    type Message = ChorusMsg;

    fn id(&self) -> ValidatorId {
        self.inner.id
    }

    fn on_start(&mut self, ctx: &mut Ctx<ChorusMsg>) {
        self.inner.on_start(ctx);
    }

    fn on_message(&mut self, ctx: &mut Ctx<ChorusMsg>, from: ValidatorId, msg: ChorusMsg) {
        self.inner.on_message(ctx, from, msg);
    }

    fn on_timer(&mut self, ctx: &mut Ctx<ChorusMsg>, timer: String) {
        debug_assert_eq!(timer_slot(&timer), Some(self.inner.slot()), "timer routed to wrong slot instance");
        self.inner.on_timer(ctx, &timer);
    }
}

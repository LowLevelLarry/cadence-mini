use crate::sim::delay::{NodeId, Tick};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TraceKind {
    Send { from: NodeId, to: NodeId, msg: String },
    Deliver { from: NodeId, to: NodeId, msg: String },
    TimerSet { node: NodeId, timer: String, fires_at: Tick },
    TimerFired { node: NodeId, timer: String },
    Custom { node: NodeId, label: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceEvent {
    pub tick: Tick,
    pub seq: u64,
    pub kind: TraceKind,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub events: Vec<TraceEvent>,
}

impl Trace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, tick: Tick, seq: u64, kind: TraceKind) {
        self.events.push(TraceEvent { tick, seq, kind });
    }

    pub fn to_canonical_json(&self) -> String {
        serde_json::to_string(&self.events).expect("trace events are always serializable")
    }
}

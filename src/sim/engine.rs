use crate::sim::delay::{DelayModel, NodeId, Tick};
use crate::sim::rng::node_rng;
use crate::sim::trace::{Trace, TraceKind};
use rand::rngs::StdRng;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

pub trait Node {
    type Message: Clone + std::fmt::Debug;

    fn id(&self) -> NodeId;
    fn on_start(&mut self, ctx: &mut Ctx<Self::Message>);
    fn on_message(&mut self, ctx: &mut Ctx<Self::Message>, from: NodeId, msg: Self::Message);
    fn on_timer(&mut self, ctx: &mut Ctx<Self::Message>, timer: String);
}

enum Payload<M> {
    Deliver { from: NodeId, msg: M },
    Timer { timer: String },
}

struct QueueItem<M> {
    tick: Tick,
    seq: u64,
    node: NodeId,
    payload: Payload<M>,
}

impl<M> PartialEq for QueueItem<M> {
    fn eq(&self, other: &Self) -> bool {
        self.tick == other.tick && self.seq == other.seq
    }
}
impl<M> Eq for QueueItem<M> {}
impl<M> PartialOrd for QueueItem<M> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<M> Ord for QueueItem<M> {
    // reversed so BinaryHeap (a max-heap) pops the smallest (tick, seq) first
    fn cmp(&self, other: &Self) -> Ordering {
        (other.tick, other.seq).cmp(&(self.tick, self.seq))
    }
}

// outbound send/timer requests a node emits during a callback; buffered so callbacks don't
// need direct access to engine internals, then drained by the engine after the callback
enum Outbound<M> {
    Send { to: NodeId, msg: M },
    Timer { after: Tick, timer: String },
}

pub struct Ctx<'a, M> {
    pub node: NodeId,
    pub tick: Tick,
    rng: &'a mut StdRng,
    outbound: Vec<Outbound<M>>,
    log: Vec<String>,
}

impl<'a, M> Ctx<'a, M> {
    pub fn send(&mut self, to: NodeId, msg: M) {
        self.outbound.push(Outbound::Send { to, msg });
    }

    pub fn broadcast(&mut self, to: &[NodeId], msg: M)
    where
        M: Clone,
    {
        for &t in to {
            self.send(t, msg.clone());
        }
    }

    pub fn set_timer(&mut self, after: Tick, timer: impl Into<String>) {
        self.outbound.push(Outbound::Timer {
            after,
            timer: timer.into(),
        });
    }

    pub fn rng(&mut self) -> &mut StdRng {
        self.rng
    }

    pub fn log(&mut self, label: impl Into<String>) {
        self.log.push(label.into());
    }
}

pub struct Engine<N: Node> {
    run_seed: u64,
    nodes: HashMap<NodeId, N>,
    node_rngs: HashMap<NodeId, StdRng>,
    queue: BinaryHeap<QueueItem<N::Message>>,
    delay_model: Box<dyn DelayModel>,
    trace: Trace,
    seq: u64,
    link_seq: HashMap<(NodeId, NodeId), u64>,
    tick: Tick,
}

impl<N: Node> Engine<N> {
    pub fn new(run_seed: u64, delay_model: Box<dyn DelayModel>) -> Self {
        Self {
            run_seed,
            nodes: HashMap::new(),
            node_rngs: HashMap::new(),
            queue: BinaryHeap::new(),
            delay_model,
            trace: Trace::new(),
            seq: 0,
            link_seq: HashMap::new(),
            tick: 0,
        }
    }

    fn next_seq(&mut self) -> u64 {
        let s = self.seq;
        self.seq += 1;
        s
    }

    pub fn add_node(&mut self, node: N) {
        let id = node.id();
        self.node_rngs.insert(id, node_rng(self.run_seed, id));
        self.nodes.insert(id, node);
    }

    pub fn node_ids(&self) -> Vec<NodeId> {
        let mut ids: Vec<_> = self.nodes.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    pub fn current_tick(&self) -> Tick {
        self.tick
    }

    fn run_callback<F>(&mut self, node_id: NodeId, tick: Tick, f: F)
    where
        F: FnOnce(&mut N, &mut Ctx<N::Message>),
    {
        let mut rng = self.node_rngs.remove(&node_id).expect("node has an rng stream");
        let mut ctx = Ctx {
            node: node_id,
            tick,
            rng: &mut rng,
            outbound: Vec::new(),
            log: Vec::new(),
        };
        let mut node = self.nodes.remove(&node_id).expect("node exists");
        f(&mut node, &mut ctx);
        let Ctx { outbound, log, .. } = ctx;
        self.nodes.insert(node_id, node);
        self.node_rngs.insert(node_id, rng);

        for label in log {
            let seq = self.next_seq();
            self.trace.record(
                tick,
                seq,
                TraceKind::Custom {
                    node: node_id,
                    label,
                },
            );
        }

        for ob in outbound {
            match ob {
                Outbound::Send { to, msg } => self.enqueue_send(node_id, to, msg, tick),
                Outbound::Timer { after, timer } => self.enqueue_timer(node_id, tick + after, timer),
            }
        }
    }

    fn enqueue_send(&mut self, from: NodeId, to: NodeId, msg: N::Message, now: Tick) {
        let link_seq = {
            let e = self.link_seq.entry((from, to)).or_insert(0);
            let v = *e;
            *e += 1;
            v
        };
        let delay = self.delay_model.delay(self.run_seed, from, to, link_seq);
        let deliver_tick = now + delay;
        let seq = self.next_seq();
        self.trace.record(
            now,
            seq,
            TraceKind::Send {
                from,
                to,
                msg: format!("{:?}", msg),
            },
        );
        let dseq = self.next_seq();
        self.queue.push(QueueItem {
            tick: deliver_tick,
            seq: dseq,
            node: to,
            payload: Payload::Deliver { from, msg },
        });
    }

    fn enqueue_timer(&mut self, node: NodeId, fires_at: Tick, timer: String) {
        let seq = self.next_seq();
        self.trace.record(
            self.tick,
            seq,
            TraceKind::TimerSet {
                node,
                timer: timer.clone(),
                fires_at,
            },
        );
        let dseq = self.next_seq();
        self.queue.push(QueueItem {
            tick: fires_at,
            seq: dseq,
            node,
            payload: Payload::Timer { timer },
        });
    }

    pub fn start(&mut self) {
        for id in self.node_ids() {
            self.run_callback(id, 0, |n, ctx| n.on_start(ctx));
        }
    }

    // runs until the queue drains or `max_tick` is exceeded (guards against runaway timers)
    pub fn run_until(&mut self, max_tick: Tick) {
        while let Some(item) = self.queue.peek() {
            if item.tick > max_tick {
                break;
            }
            let item = self.queue.pop().unwrap();
            self.tick = item.tick;
            match item.payload {
                Payload::Deliver { from, msg } => {
                    let seq = self.next_seq();
                    self.trace.record(
                        item.tick,
                        seq,
                        TraceKind::Deliver {
                            from,
                            to: item.node,
                            msg: format!("{:?}", msg),
                        },
                    );
                    let tick = item.tick;
                    self.run_callback(item.node, tick, |n, ctx| n.on_message(ctx, from, msg));
                }
                Payload::Timer { timer } => {
                    let seq = self.next_seq();
                    self.trace.record(
                        item.tick,
                        seq,
                        TraceKind::TimerFired {
                            node: item.node,
                            timer: timer.clone(),
                        },
                    );
                    let tick = item.tick;
                    self.run_callback(item.node, tick, |n, ctx| n.on_timer(ctx, timer));
                }
            }
        }
    }

    pub fn node(&self, id: NodeId) -> Option<&N> {
        self.nodes.get(&id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &N> {
        self.nodes.values()
    }
}

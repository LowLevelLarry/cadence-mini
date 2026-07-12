// a configured link delay should show up exactly in the trace.

use cadence_mini::sim::{Ctx, Engine, FixedDelay, Node, NodeId, TraceKind};

#[derive(Clone, Debug)]
struct Hello;

struct Speaker {
    id: NodeId,
    peer: NodeId,
}

impl Node for Speaker {
    type Message = Hello;

    fn id(&self) -> NodeId {
        self.id
    }

    fn on_start(&mut self, ctx: &mut Ctx<Hello>) {
        if self.id == 0 {
            ctx.send(self.peer, Hello);
        }
    }

    fn on_message(&mut self, _ctx: &mut Ctx<Hello>, _from: NodeId, _msg: Hello) {}
    fn on_timer(&mut self, _ctx: &mut Ctx<Hello>, _timer: String) {}
}

#[test]
fn configured_delay_observed_in_trace() {
    let mut engine: Engine<Speaker> = Engine::new(7, Box::new(FixedDelay(50)));
    engine.add_node(Speaker { id: 0, peer: 1 });
    engine.add_node(Speaker { id: 1, peer: 0 });
    engine.start();
    engine.run_until(200);

    let send_tick = engine
        .trace()
        .events
        .iter()
        .find_map(|e| match &e.kind {
            TraceKind::Send { from: 0, to: 1, .. } => Some(e.tick),
            _ => None,
        })
        .expect("node 0 sends Hello to node 1");

    let deliver_tick = engine
        .trace()
        .events
        .iter()
        .find_map(|e| match &e.kind {
            TraceKind::Deliver { from: 0, to: 1, .. } => Some(e.tick),
            _ => None,
        })
        .expect("node 1 delivers Hello from node 0");

    assert_eq!(
        deliver_tick - send_tick,
        50,
        "the configured 50-tick delay must be exactly reflected between send and delivery"
    );
}

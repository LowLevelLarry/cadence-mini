// GATE 1a — determinism: same seed run twice produces byte-identical traces, 20 seeds.

use cadence_mini::sim::{Ctx, Engine, FixedDelay, Node, NodeId, UniformDelay};
use rand::Rng;

#[derive(Clone, Debug)]
enum Msg {
    Ping(u64),
    Pong(u64),
}

struct Pinger {
    id: NodeId,
    peers: Vec<NodeId>,
    round: u64,
}

impl Node for Pinger {
    type Message = Msg;

    fn id(&self) -> NodeId {
        self.id
    }

    fn on_start(&mut self, ctx: &mut Ctx<Msg>) {
        // use the rng to prove the recorded trace also captures rng-influenced behavior
        let jitter = ctx.rng().next_u32() % 5;
        ctx.set_timer(1 + jitter as u64, "kickoff");
    }

    fn on_message(&mut self, ctx: &mut Ctx<Msg>, from: NodeId, msg: Msg) {
        match msg {
            Msg::Ping(r) => {
                ctx.log(format!("got ping {r} from {from}"));
                ctx.send(from, Msg::Pong(r));
            }
            Msg::Pong(r) => {
                ctx.log(format!("got pong {r} from {from}"));
            }
        }
    }

    fn on_timer(&mut self, ctx: &mut Ctx<Msg>, timer: String) {
        if timer == "kickoff" && self.round < 3 {
            self.round += 1;
            for &p in self.peers.clone().iter() {
                ctx.send(p, Msg::Ping(self.round));
            }
            ctx.set_timer(10, "kickoff");
        }
    }
}

fn run_with_seed(seed: u64) -> String {
    let delay = UniformDelay { min: 1, max: 7 };
    let mut engine: Engine<Pinger> = Engine::new(seed, Box::new(delay));
    let ids: Vec<NodeId> = (0..5).collect();
    for &id in &ids {
        let peers: Vec<NodeId> = ids.iter().copied().filter(|&p| p != id).collect();
        engine.add_node(Pinger { id, peers, round: 0 });
    }
    engine.start();
    engine.run_until(200);
    engine.trace().to_canonical_json()
}

#[test]
fn same_seed_same_trace() {
    for seed in 0..20u64 {
        let a = run_with_seed(seed);
        let b = run_with_seed(seed);
        assert_eq!(a, b, "trace mismatch for seed {seed}");
    }
}

#[test]
fn different_seeds_generally_differ() {
    // sanity check that the harness isn't accidentally constant-folding everything away
    let a = run_with_seed(1);
    let b = run_with_seed(2);
    assert_ne!(a, b);
}

#[test]
fn fixed_delay_is_deterministic_too() {
    let mut engine: Engine<Pinger> = Engine::new(42, Box::new(FixedDelay(5)));
    let ids: Vec<NodeId> = (0..3).collect();
    for &id in &ids {
        let peers: Vec<NodeId> = ids.iter().copied().filter(|&p| p != id).collect();
        engine.add_node(Pinger { id, peers, round: 0 });
    }
    engine.start();
    engine.run_until(100);
    let a = engine.trace().to_canonical_json();

    let mut engine2: Engine<Pinger> = Engine::new(42, Box::new(FixedDelay(5)));
    for &id in &ids {
        let peers: Vec<NodeId> = ids.iter().copied().filter(|&p| p != id).collect();
        engine2.add_node(Pinger { id, peers, round: 0 });
    }
    engine2.start();
    engine2.run_until(100);
    let b = engine2.trace().to_canonical_json();

    assert_eq!(a, b);
}

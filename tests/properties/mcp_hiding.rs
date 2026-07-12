// hiding: no proposer's proposal-construction event should causally follow another
// proposer's proposal-broadcast for the same slot. Checked structurally against the trace:
// for every ordered pair of proposers (i, j), the tick at which i *sends* its own
// Disseminate must not be later than the tick at which i *receives* (delivers) j's
// Disseminate — if it were later, i could in principle have built its proposal in reaction
// to j's, which is exactly what hiding rules out.

use crate::chorus_common::*;
use cadence_mini::chorus::ProposerBehavior;
use cadence_mini::sim::TraceKind;
use std::collections::HashMap;

#[test]
fn no_causal_dependency_between_proposals() {
    let proposers = vec![0u64, 1, 2];
    let config = base_config(proposers.clone());
    let behavior = HashMap::from([
        (0, ProposerBehavior::Honest),
        (1, ProposerBehavior::Honest),
        (2, ProposerBehavior::Honest),
    ]);
    let delay = synchronous_delay(5, 11);
    let mut engine = build_engine(config, behavior, delay, 11);
    engine.start();
    engine.run_until(200);

    let trace = engine.trace();

    for &i in &proposers {
        let send_tick = trace
            .events
            .iter()
            .find_map(|e| match &e.kind {
                TraceKind::Send { from, to, msg } if *from == i && *to == i && msg.contains("Disseminate") => {
                    Some(e.tick)
                }
                _ => None,
            })
            // a validator's own dissemination is sent to itself too (broadcast includes self)
            .unwrap_or_else(|| panic!("proposer {i} never disseminated"));

        for &j in &proposers {
            if i == j {
                continue;
            }
            // first tick at which i received (delivered) j's Disseminate message
            if let Some(deliver_tick) = trace.events.iter().find_map(|e| match &e.kind {
                TraceKind::Deliver { from, to, msg } if *from == j && *to == i && msg.contains("Disseminate") => {
                    Some(e.tick)
                }
                _ => None,
            }) {
                assert!(
                    send_tick <= deliver_tick,
                    "hiding violated: proposer {i} sent its own proposal at tick {send_tick}, \
                     after already having received proposer {j}'s proposal at tick {deliver_tick}"
                );
            }
        }
    }
}

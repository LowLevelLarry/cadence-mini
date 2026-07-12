# notes

Working notes from building cadence-mini, a simulation-first implementation of Cadence
("Cadence: Extreme Pipelining with Multiple Concurrent Proposers", arXiv 2607.02275). Not a
polished writeup — this is where I dumped the paper's structure while reading it and logged
every place I had to make a call the paper doesn't spell out for a toy simulator.

## Paper digest

Cadence is a BFT consensus protocol built around two ideas:

- **Extreme pipelining (§3, §5).** Slot s+1 doesn't wait on slot s to finish. A Conductor
  orchestrator opens slots in windows of size W, and only requires that p out of the current
  window's W slots have completed before opening the next window — so at most `2W - p` slots
  can be outstanding for any honest validator at once. This is a purely local bound; no
  cross-validator coordination is needed to enforce it beyond agreeing on window deadlines.
- **Multiple concurrent proposers, MCP (§4.1/§4.4/§6.2).** Instead of one leader per slot,
  k proposers each submit independently. A slot's meta-block is a vector of per-proposer
  entries (Included(digest) or Excluded), certified together in the *same* two voting rounds
  a single-proposer protocol would need — MCP isn't a phase bolted onto single-leader
  consensus, it falls out of treating the proposer set as a vector from the start. Recovery
  merges the included proposals deterministically (§6.2): concatenate in proposer order,
  dedupe transactions keeping first occurrence.

Per-slot consensus itself (Chorus, §4) has a fast path and a fallback path:

- **Fast path (optimal 3 rounds):** proposers disseminate, validators cast one round-1 vote
  per proposer at a fixed deadline, and once a validator sees 2f+1 round-1 votes for every
  proposer it has *speculative finality* (§4.2) and casts a second round vote on the whole
  assembled meta-block. 2f+1 of those second-round votes gives full finality — one round
  earlier for speculative than for full.
- **Fallback path (§4.3):** if the fast path stalls (partial dissemination, a slow proposer,
  equivocation), validators time out and fall back to a slower path — a black-box asynchronous
  agreement problem in the paper, which every honest validator's fallback bundle feeds into.
- **Equivocation (§4.3/§4.5):** if a proposer sends conflicting proposals, two groups of at
  least f+1 validators can end up attesting to different digests for the same (slot,
  proposer). That's the equivocation proof, and it's the only thing that licenses reverting a
  speculative finalization.

## Where I deviated from the paper, and why

Numbered so REPORT.md and code comments can point back at a specific entry.

1. **Crypto is a mocked trait, permanently.** `Certifier` counts `(validator, vote)` pairs
   against a threshold; there's no real signing, hashing for security, or erasure coding
   anywhere. This was a hard constraint going in, not something I discovered had to be faked —
   listing it here for completeness since everything else in this list assumes it.

2. **The fallback agreement is a simulation-only stand-in**, not a general async BFT/ACS
   protocol. The paper treats it as black-box; I implemented a deterministic per-slot leader
   that collects fallback proposals and decides by max-multiplicity (canonical tie-break),
   backed by an echo round for a real 2f+1 certificate. See
   `src/chorus/instance.rs::maybe_lead_fallback_agreement`.

3. **Conductor's deadline agreement is local, not the paper's real ACS+median sub-protocol.**
   The simulator already gives every honest validator an identical, deterministic view of
   which slots have completed, so each validator computes its own next-window deadline
   directly instead of running a separate agreement instance to converge on one. See
   `src/pipeline.rs::maybe_advance`.

4. **Equivocation-vote tie resolution had a real nondeterminism bug**, not just a design
   choice. `compute_fallback_vote` originally picked "the first digest that clears f+1"
   straight out of a `HashMap`, whose iteration order is randomized per process — so two
   validators could pick *different* digests out of a genuine 2-way equivocation, which is a
   safety violation in disguise, not just noisy test output. Fixed by grouping votes by digest
   first, sorting canonically, and checking for >=2 "strong" groups (an actual tie) before
   ever accepting a single value. See `src/chorus/instance.rs::compute_fallback_vote`.

5. **A corollary of #2: live reverts are structurally unreachable with this fallback-agreement
   rule.** With the leader's max-multiplicity vote, a fast-quorum-backed meta-block (held by
   >= 2f+1 of n=3f+1 validators) is provably always a plurality of any 2f+1-sized batch the
   leader collects — a minority proposal can never mathematically outvote it. So no validator
   can ever experience a live "speculative view overturned by full finalization" in this
   implementation, even though the paper's proof only claims reverts are *sound* (backed by a
   proof), not that they can't happen. I tested the conditional property instead — see
   `tests/properties/speculative_revert.rs`'s file-level comment for the full argument and the
   2-vs-2 split scenario that exercises detection without needing a live revert.

6. **The 5-region geo delay matrix for the latency experiment is invented to be
   geo-plausible**, not Monad's actual measured inter-validator latency (which isn't published
   in a reproducible form). `experiments/latency.rs` and `REPORT.md` are explicit that only the
   *shape* of the result (speculative substantially faster than final, ratio in the same
   neighborhood as the paper's ~0.76) is being compared, not absolute numbers. See
   `src/adversary/geo.rs`.

7. **No execution layer.** A "block" is a merged, deduplicated, ordered list of opaque `u64`
   values standing in for transactions (`types::merge_block`, §6.2's merge rule). Nothing gets
   executed; this is purely about consensus on ordering and inclusion.

## 2026-07-12 — latency measurement was anchored to the wrong clock

Spent a while chasing why the latency experiment's speculative/final ratio came out at ~0.48
against the paper's ~0.76. Turned out to be a bug in what the "zero" of the measurement was,
not a protocol bug.

`experiments/latency.rs` was computing both `finalization_latencies` and
`speculative_latencies` as `tick - DEADLINE`, where `DEADLINE` is the fixed per-slot round-1
voting deadline (a configured buffer, `delta=250ms` past slot start, sized to comfortably
absorb dissemination). But dissemination itself finishes *before* the deadline in a healthy
run — so anchoring to the deadline was silently throwing away the entire dissemination-delay
component from both numbers. Since real network dissemination delay (call it D) is nonzero and
roughly comparable in size to a single voting round here, dropping it doesn't just shift both
numbers by a constant — it drags the ratio toward the "pure two-rounds-vs-one-round" limit of
0.5, away from the paper's D-dominated ~0.76 (where D is large relative to a single round, so
speculative and final latency are closer together).

Added per-validator instrumentation (`last_dissemination_tick` on `ChorusInstance`) to
decompose the trace into: D = last accepted dissemination minus slot start, round-1 gap, and
round-2 gap. The round-2 gap (speculative tick to full-finality tick) is the cleanest available
estimate of a single voting round, since both endpoints are genuinely network-driven events
rather than a fixed timer — confirmed the fast path really is exactly two voting rounds past
dissemination, matching the paper; the round count was never the problem.

Fixed by re-anchoring both latencies to slot start instead of the deadline. Ratio moved from
~0.48 to ~0.71 — into the paper's band. Full numbers and the decomposition table are in
REPORT.md.

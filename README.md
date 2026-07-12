# cadence-mini

A small, simulation-first implementation of Cadence, the multiple-concurrent-proposer (MCP)
BFT consensus protocol from Category Labs' paper "Cadence: Extreme Pipelining with Multiple
Concurrent Proposers" (arXiv 2607.02275). I built this as a learning/reference project — a
way to hold the paper in one hand and working code in the other, not a production node.

Requires Rust 1.88+ (edition 2024 with let-chains, which stabilized in 1.88).

## What's actually in here

Everything runs inside a single-threaded, seeded, discrete-event simulator (`src/sim/`): a
logical clock, a priority queue of events, an injectable per-link delay model, and a full
trace recorder. No threads, no sockets, no wall clock — same seed always produces a
byte-identical trace. Cryptography is a trait (`Certifier`) with one mock implementation that
just counts `(validator, vote)` sets against stake thresholds; there's no real signing, hashing
for real security properties, or erasure coding anywhere.

On top of that:

- **`chorus/`** — the actual slot consensus protocol: proposers disseminate, validators cast
  round-1 votes per proposer, a fast meta-block gets assembled and speculatively finalized,
  and a second round of votes gives full finality in the optimal 3 message-rounds. When the
  fast path can't complete (partial dissemination, a slow proposer, equivocation), validators
  fall back to a slower path that still safely finalizes.
- **`pipeline.rs`** — the extreme-pipelining framework: many `ChorusInstance`s run
  concurrently inside one validator, one per open slot, with no instance waiting on another.
  A simplified Conductor throttles how many slots can be open at once.
- **`mcp.rs`** / the meta-block structure in `chorus/` — multiple proposers per slot fold
  into the *same* two voting rounds; there's no separate aggregation phase.
- **`speculative.rs`** — typed outcomes for a slot's speculative-vs-final view, and the logic
  that checks a reverted speculation is always backed by a real equivocation proof.
- **`adversary/`** — an equivocator, a censor, an offline/muted validator, a partition-and-heal
  delay model, and a geo-plausible delay matrix for the latency experiment.

Property tests live under `tests/properties/`, most run across a spread of seeds.

## What's simplified (the honest list)

- **No real cryptography.** Signatures, Merkle commitments, erasure coding, and threshold
  encryption are all replaced by the mock `Certifier` and by treating a "proposal" as a plain
  value. The *structure* those primitives enable (thresholds, availability, hiding) is
  preserved; the primitives themselves aren't real.
- **No execution layer.** A "block" here is just a merged, deduplicated, ordered list of
  opaque `u64` values standing in for transactions. Nothing gets executed.
- **The fallback agreement is a simulation-only stand-in**, not a general asynchronous
  BFT/ACS protocol: a deterministic per-slot leader collects proposals and decides by
  max-multiplicity, backed by an echo round for a real certificate. This turns out to make one
  specific thing structurally unreachable here that the real protocol only makes *rare*: a
  live "speculative view actually overturned by full finalization" event, because a
  fast-quorum-backed meta-block is provably always a plurality of anything this leader rule
  collects. The revert test checks *soundness* instead (if a revert happens, it's justified by
  a real equivocation proof) rather than forcing a live one.
- **Conductor's deadline agreement is local, not a real ACS+median sub-protocol.** The
  simulator gives every honest validator the same deterministic view of completions, so each
  validator computes its own next-window deadline directly rather than running a separate
  agreement instance to converge on one.
- **The latency experiment compares shape, not numbers.** The 200-validator, 5-region delay
  matrix is invented to be geo-plausible; it isn't Monad's actual measured inter-validator
  latency, so absolute numbers aren't expected to match. `REPORT.md` includes a round-by-round
  decomposition (dissemination delay, then each voting round, measured from slot start) — the
  speculative/final ratio it produces lands close to the paper's ~0.76.

## Running it

```
cargo test --test properties      # the full property-test suite, one binary
cargo clippy --all-targets        # no lint warnings anywhere
cargo run --release --bin latency # regenerates REPORT.md
```

## What I'd do next

Wire a real ACS instance into Conductor's window transitions instead of the local
approximation, replace the fallback-agreement leader rule with something that can't
structurally rule out live reverts, and — if I wanted this to be more than a reference —
start thinking about the execution layer and real cryptography this deliberately leaves out.

#!/usr/bin/env bash
# mutation testing: applies a handful of targeted, meaning-breaking patches one at a time,
# confirms the test suite actually notices (the named test must FAIL), then restores the
# file. this is the only automated proof that the property tests have teeth rather than just
# being green by accident.
set -euo pipefail
cd "$(dirname "$0")/.."

declare -a NAMES=()
declare -a RESULTS=()

# apply.py FILE FIND REPLACE — exact, whitespace-sensitive substring substitution. fails
# loudly (nonzero exit) if FIND isn't found exactly once, so a mutation can't silently
# no-op because the source moved out from under it.
apply() {
    python3 - "$1" "$2" "$3" <<'EOF'
import sys
path, find, replace = sys.argv[1], sys.argv[2], sys.argv[3]
src = open(path).read()
n = src.count(find)
if n != 1:
    sys.exit(f"expected exactly one occurrence of the target text in {path}, found {n}")
open(path, "w").write(src.replace(find, replace, 1))
EOF
}

cleanup() {
    git checkout -- src/certifier.rs src/speculative.rs src/chorus/instance.rs src/pipeline.rs 2>/dev/null || true
}
trap cleanup EXIT

run_mutation() {
    local name="$1" file="$2" expect_test="$3"
    echo "== mutation: $name =="

    set +e
    cargo test --release --test properties "$expect_test" -- --exact >/tmp/mutation_out.txt 2>&1
    local status=$?
    set -e

    git checkout -- "$file"

    NAMES+=("$name")
    if [ "$status" -ne 0 ] && grep -q "FAILED" /tmp/mutation_out.txt; then
        echo "KILLED (test '$expect_test' failed, as expected)"
        RESULTS+=("KILLED")
    else
        echo "SURVIVED (test '$expect_test' did not fail — mutation went undetected)"
        RESULTS+=("SURVIVED")
    fi
    rm -f /tmp/mutation_out.txt
    echo
}

# mutation 1: quorum weakening (2f+1 -> f+1) in the certifier. this is the threshold every
# safety-critical certificate check (fast finalization, fallback echo) goes through, so
# weakening it should immediately let two conflicting values both certify.
apply src/certifier.rs \
    "        2 * self.f() + 1" \
    "        self.f() + 1"
run_mutation \
    "quorum weakening (2f+1 -> f+1)" \
    "src/certifier.rs" \
    "chorus_safety::no_conflicting_finalization"

# mutation 2: proof laundering — justifying_proof returns whatever proof happens to be first,
# instead of the one that actually implicates the proposer whose entry changed.
apply src/speculative.rs \
    "    proofs.iter().find(|proof| changed_proposers.contains(&proof.proposer))" \
    "    proofs.iter().next()"
run_mutation \
    "proof laundering (justifying_proof ignores which proposer changed)" \
    "src/speculative.rs" \
    "speculative_revert::justifying_proof_matches_the_changed_proposer"

# mutation 3: hiding violation — the last-listed proposer skips its own scheduled
# dissemination and instead waits to see another proposer's Disseminate before sending its
# own (one tick later), i.e. its proposal construction causally follows another proposer's
# broadcast instead of being independent of it.
apply src/chorus/instance.rs \
    "        if self.is_proposer() {
            let start = self.config.start_time();
            if ctx.tick >= start {
                self.disseminate(ctx);
            } else {
                ctx.set_timer(start - ctx.tick, timer_name(slot, \"disseminate\"));
            }
        }" \
    "        if self.is_proposer() && self.config.proposers.last() != Some(&self.id) {
            let start = self.config.start_time();
            if ctx.tick >= start {
                self.disseminate(ctx);
            } else {
                ctx.set_timer(start - ctx.tick, timer_name(slot, \"disseminate\"));
            }
        }"
apply src/chorus/instance.rs \
    "                self.received.entry(proposer).or_insert_with(|| (crate::types::digest_of(&payload), payload));
            }
            ChorusMsg::Round1Vote" \
    "                self.received.entry(proposer).or_insert_with(|| (crate::types::digest_of(&payload), payload));
                if self.config.proposers.last() == Some(&self.id) && !self.received.contains_key(&self.id) {
                    ctx.set_timer(1, timer_name(slot, \"disseminate\"));
                }
            }
            ChorusMsg::Round1Vote"
run_mutation \
    "hiding violation (last proposer waits to see another's dissemination first)" \
    "src/chorus/instance.rs" \
    "mcp_hiding::no_causal_dependency_between_proposals"

# mutation 4: throttle removal — the window-advance gate always thinks the threshold's worth
# of the current window has completed, so windows open back-to-back with no bound.
apply src/pipeline.rs \
    "    fn window_first_p_complete(&self, window: usize) -> bool {
        self.cfg
            .window_slots(window)
            .take(self.cfg.threshold)
            .all(|s| self.completed.contains(&s))
    }" \
    "    fn window_first_p_complete(&self, _window: usize) -> bool {
        true
    }"
run_mutation \
    "throttle removal (window_first_p_complete always true)" \
    "src/pipeline.rs" \
    "pipeline_throttling::outstanding_never_exceeds_bound"

echo "=================================="
echo "mutation summary:"
all_killed=true
for i in "${!NAMES[@]}"; do
    printf '  %-70s %s\n' "${NAMES[$i]}" "${RESULTS[$i]}"
    if [ "${RESULTS[$i]}" != "KILLED" ]; then
        all_killed=false
    fi
done
echo "=================================="

dirty="$(git status --porcelain -- src/)"
if [ -n "$dirty" ]; then
    echo "src/ is dirty after mutation testing — a restore step failed" >&2
    echo "$dirty" >&2
    exit 1
fi

if [ "$all_killed" = false ]; then
    echo "one or more mutations survived — the corresponding test is not actually testing this" >&2
    exit 1
fi

echo "all mutations killed, tree clean."

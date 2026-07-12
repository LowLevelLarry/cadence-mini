#!/usr/bin/env python3
"""one-off check: every module::test_fn name in SPEC.md matches a real #[test] fn."""
import re

spec = open("SPEC.md").read()
names = re.findall(r"`([a-z0-9_]+::[a-z0-9_]+)`", spec)

missing = []
for n in names:
    mod, fn = n.split("::")
    path = f"tests/properties/{mod}.rs"
    try:
        content = open(path).read()
    except FileNotFoundError:
        missing.append((n, f"{path} does not exist"))
        continue
    if not re.search(r"fn\s+" + re.escape(fn) + r"\s*\(", content):
        missing.append((n, f"fn {fn} not found in {path}"))

print(f"{len(names)} test names checked")
if missing:
    print("MISSING:")
    for n, reason in missing:
        print(f"  {n}: {reason}")
    raise SystemExit(1)
print("all SPEC.md test names resolve to real #[test] functions")

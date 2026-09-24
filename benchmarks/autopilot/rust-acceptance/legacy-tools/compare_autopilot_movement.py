#!/usr/bin/env python3
"""Compare deterministic encounter summaries; timing belongs to autopilot-bench."""
import argparse
import json
from pathlib import Path


def indexed(path):
    """Reject duplicate fixture identities rather than silently discarding a run."""
    rows = json.loads(Path(path).read_text())
    result = {(row["scenario"], row["chassis"], row["seed"]): row for row in rows}
    assert len(result) == len(rows), "duplicate encounter identity"
    return result


def compare(baseline, candidate):
    """Enforce outcome tolerances and report meaningful movement improvements."""
    assert baseline.keys() == candidate.keys(), "encounter matrices differ"
    failures = []
    for key, after in candidate.items():
        before = baseline[key]
        assert before["ticks"] == after["ticks"], "encounter durations differ"
        first, second = before["time_to_engage"], after["time_to_engage"]
        if first is not None and (second is None or second > first + 1):
            failures.append(f"{key}: first shot regressed: {first} -> {second}")
        if after["arc_fraction"] + 0.02 < before["arc_fraction"]:
            failures.append(f"{key}: usable arcs regressed by more than two percentage points")
        if after["state"] == "blocked":
            failures.append(f"{key}: controller blocked")
        # Total travel bounds every contained twenty-tick window as well.
        if after["settled_distance"] > 0.25:
            failures.append(f"{key}: more than 0.25 hex traveled after settling")
        if key[0] not in ("moving", "attack_move") and after["settled_ticks"] < 20:
            failures.append(f"{key}: fewer than twenty settled ticks")
    return {
        "encounters": len(candidate),
        "committed_ticks_per_trace": sum(row["ticks"] for row in candidate.values()),
        "newly_engaged": sum(
            baseline[key]["time_to_engage"] is None and row["time_to_engage"] is not None
            for key, row in candidate.items()
        ),
        "max_settled_distance": max(row["settled_distance"] for row in candidate.values()),
        "failures": failures,
    }


def main():
    """Write a machine-readable comparison and fail on any acceptance regression."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline")
    parser.add_argument("candidate")
    args = parser.parse_args()
    report = compare(indexed(args.baseline), indexed(args.candidate))
    print(json.dumps(report, indent=2))
    raise SystemExit(bool(report["failures"]))


if __name__ == "__main__":
    main()

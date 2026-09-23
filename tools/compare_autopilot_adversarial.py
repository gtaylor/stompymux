#!/usr/bin/env python3
"""Compare per-participant adversarial metrics without treating casualties as settled units."""
import argparse
import json
from pathlib import Path


def indexed(path):
    """Require stable schema and unique named participants."""
    rows = json.loads(Path(path).read_text())
    assert rows and all(row["schema"] == 1 for row in rows)
    values = {(r["scenario"], r["chassis"], r["seed"], r["role"]): r for r in rows}
    assert len(rows) == len(values), "duplicate participant"
    return values


def compare(before, after):
    """Report outcome failures explicitly; impossible traffic is diagnostic."""
    assert before.keys() == after.keys(), "participant matrices differ"
    failures, limitations, changes = [], [], []
    for key, b in before.items():
        a = after[key]
        assert a["ticks"] == b["ticks"]
        scenario, _, _, role = key
        if scenario == "no_passing_space":
            if role == "focal":
                limitations.append({"participant": key, "outcome": a["outcome"], "reason": a["blocking_reason"]})
            continue
        controlled = (scenario in ("bottleneck", "late_clearance", "alternating_clearance", "waiting_controllers") and (role == "focal" or role.startswith("waiting_"))) or (scenario == "passage" and role in ("focal", "opponent"))
        if controlled and a["outcome"] != "completed":
            failures.append(f"{key}: reachable movement did not complete: {a['outcome']}")
        if scenario == "bottleneck" and role == "focal" and a["recovery_ticks"] is None:
            failures.append(f"{key}: no measured recovery after clearance")
        if controlled and scenario in ("late_clearance", "alternating_clearance", "waiting_controllers"):
            clearance = a.get("congestion") or {}
            delay = clearance.get("clearance_to_search")
            if delay is None or delay > 5:
                failures.append(f"{key}: watched-cell clearance did not start a search within five ticks")
            if scenario == "late_clearance":
                old = (b.get("congestion") or {}).get("clearance_to_search")
                # Require improvement over timed-only recovery, not over an already
                # responsive baseline in future regression comparisons.
                timed_only = (b.get("congestion") or {}).get("early_starts", 0) == 0
                if timed_only and old is not None and old > 5 and delay is not None and delay >= old:
                    failures.append(f"{key}: late-clearance retry delay did not improve")
        expected_expiry = (scenario == "occluded" and a["blocking_reason"] == "ContactLost"
                           and a["first_shot"] is not None and a.get("longest_contact_gap", 0) >= 30)
        if expected_expiry:
            limitations.append({"participant": key, "outcome": "contact_expiry", "longest_contact_gap": a["longest_contact_gap"], "reacquisitions": a.get("reacquisitions", 0)})
        if a["outcome"] == "blocked" and scenario == "duel":
            limitations.append({"participant": key, "outcome": "combat_blocked", "reason": a["blocking_reason"]})
        if a["outcome"] == "blocked" and scenario != "duel" and not expected_expiry:
            failures.append(f"{key}: unexpected blocking: {a['blocking_reason']}")
        combatant = (role == "focal" and not controlled) or role.startswith("pursuer")
        if combatant and a["first_shot"] is None and a["outcome"] != "destroyed":
            failures.append(f"{key}: did not engage")
        # Duels/casualties change exposure: report separately, don't mislabel lower uptime.
        if combatant and scenario != "duel" and a["outcome"] != "destroyed" and b["outcome"] != "destroyed":
            if b["first_shot"] is not None and (a["first_shot"] is None or a["first_shot"] > b["first_shot"] + 1):
                failures.append(f"{key}: first-shot regression")
            if a["arc_fraction"] + .02 < b["arc_fraction"]:
                failures.append(f"{key}: arc uptime regression")
        if a != b:
            changes.append({"participant": key, "before": b, "after": a})
    cases = {key[:3]: value["ticks"] for key, value in after.items()}
    completed = lambda rows: sum(r["outcome"] == "completed" for k, r in rows.items() if k[0] in ("bottleneck", "passage") and k[3] == "focal")
    return {"encounters": len(cases), "committed_ticks_per_trace": sum(cases.values()), "participants": len(after), "controlled_completions_before": completed(before), "controlled_completions_after": completed(after), "failures": failures, "limitations": limitations, "changes": changes}


def main():
    """Emit JSON and fail rather than hiding unmet behavior gates."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline")
    parser.add_argument("candidate")
    args = parser.parse_args()
    result = compare(indexed(args.baseline), indexed(args.candidate))
    print(json.dumps(result, indent=2))
    raise SystemExit(bool(result["failures"]))


if __name__ == "__main__":
    main()

"""Small schema and behavioral-gate regressions; no game database or heartbeat."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from compare_autopilot_adversarial import compare, indexed


class ComparisonTests(unittest.TestCase):
    def row(self, scenario="crossing"):
        return {"schema": 1, "scenario": scenario, "chassis": "mech", "seed": 1,
                "role": "focal", "ticks": 240, "sampled_ticks": 240,
                "first_shot": 10, "arc_fraction": .8, "outcome": "window_end",
                "blocking_reason": None, "recovery_ticks": None}

    def pair(self, before, after):
        key = (before["scenario"], "mech", 1, before["role"])
        return compare({key: before}, {key: after})

    def test_duplicate_roles_are_rejected(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "rows.json"
            path.write_text(json.dumps([self.row(), self.row()]))
            with self.assertRaises(AssertionError):
                indexed(path)

    def test_first_shot_regression_fails(self):
        before = self.row()
        after = copy.deepcopy(before)
        after["first_shot"] = 12
        self.assertTrue(self.pair(before, after)["failures"])

    def test_casualty_exposure_is_not_arc_regression(self):
        before = self.row("duel")
        after = copy.deepcopy(before)
        after.update(outcome="destroyed", sampled_ticks=20, arc_fraction=.1)
        self.assertFalse(self.pair(before, after)["failures"])

    def test_bottleneck_requires_completion_and_recovery(self):
        before = self.row("bottleneck")
        self.assertEqual(len(self.pair(before, before)["failures"]), 2)
        after = copy.deepcopy(before)
        after.update(outcome="completed", recovery_ticks=1)
        self.assertFalse(self.pair(before, after)["failures"])

    def test_impossible_passage_is_explicit_diagnostic(self):
        row = self.row("no_passing_space")
        row.update(outcome="blocked", blocking_reason="Unreachable")
        result = self.pair(row, row)
        self.assertFalse(result["failures"])
        self.assertEqual(len(result["limitations"]), 1)

    def test_contact_expiry_requires_measured_loss_after_engagement(self):
        row = self.row("occluded")
        row.update(outcome="blocked", blocking_reason="ContactLost", longest_contact_gap=10)
        self.assertTrue(self.pair(row, row)["failures"])
        row["longest_contact_gap"] = 31
        result = self.pair(row, row)
        self.assertFalse(result["failures"])
        self.assertEqual(result["limitations"][0]["outcome"], "contact_expiry")

    def test_opposing_passage_requires_both_roles_to_finish(self):
        row = self.row("passage")
        row["role"] = "opponent"
        self.assertTrue(self.pair(row, row)["failures"])
        row["outcome"] = "completed"
        self.assertFalse(self.pair(row, row)["failures"])

    def test_clearance_requires_prompt_search_and_late_case_improvement(self):
        before = self.row("late_clearance")
        before.update(outcome="completed", congestion={"clearance_to_search": 20})
        after = copy.deepcopy(before)
        after["congestion"]["clearance_to_search"] = 4
        self.assertFalse(self.pair(before, after)["failures"])
        after["congestion"]["clearance_to_search"] = 6
        self.assertTrue(self.pair(before, after)["failures"])

    def test_responsive_baseline_can_remain_unchanged(self):
        row = self.row("late_clearance")
        row.update(outcome="completed", congestion={"clearance_to_search": 3, "early_starts": 1})
        self.assertFalse(self.pair(row, copy.deepcopy(row))["failures"])

    def test_matrix_identity_must_match(self):
        with self.assertRaises(AssertionError):
            compare({}, {("crossing", "mech", 1, "focal"): self.row()})


if __name__ == "__main__":
    unittest.main()

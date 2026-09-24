"""Pure comparator tests; no slow encounter fixtures."""
import copy
import unittest
from compare_autopilot_pursuit import indexed, compare

class ComparisonTests(unittest.TestCase):
    def row(self):
        return dict(schema=1, scenario='lateral', chassis='mech', seed=1, fire=False,
                    first_ready=100, approach_distance=10., ready_ticks=10,
                    visible_ticks=100, outcome='window_end', episodes=[])

    def test_improvement(self):
        b=self.row(); a=copy.deepcopy(b); a.update(first_ready=90,approach_distance=9.)
        self.assertFalse(compare(indexed([b]),indexed([a]))['failures'])

    def test_unchanged_is_not_improvement(self):
        r=self.row()
        self.assertTrue(compare(indexed([r]),indexed([r]))['failures'])

    def test_missing_milestone_is_not_zero_cost(self):
        b=self.row(); a=copy.deepcopy(b); a.update(first_ready=None,approach_distance=None,outcome='destroyed')
        r=compare(indexed([b]),indexed([a]))
        self.assertTrue(r['failures']); self.assertTrue(r['unmatched']); self.assertFalse(r['paired_excess_travel'])

    def test_duplicate_rejected(self):
        with self.assertRaises(AssertionError): indexed([self.row(),self.row()])

    def test_early_opportunity_does_not_hide_later_blocking(self):
        b=self.row(); a=copy.deepcopy(b)
        a.update(first_ready=90,approach_distance=9.,outcome='Stuck')
        failures=compare(indexed([b]),indexed([a]))['failures']
        self.assertTrue(any('sustained-pursuit' in f['reason'] for f in failures))

    def test_existing_blocking_is_still_a_failed_feasible_encounter(self):
        b=self.row(); b.update(scenario='retreat',outcome='Stuck')
        failures=compare(indexed([b]),indexed([b]))['failures']
        self.assertTrue(any('sustained-pursuit' in f['reason'] for f in failures))

    def test_matrix_mismatch(self):
        with self.assertRaises(AssertionError): compare({},indexed([self.row()]))

if __name__=='__main__': unittest.main()

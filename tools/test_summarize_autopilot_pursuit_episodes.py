"""Small attribution tests, independent of the simulation harness."""
import unittest
from summarize_autopilot_pursuit_episodes import COUNTERS, summarize


class EpisodeTests(unittest.TestCase):
    def fixture(self):
        row = dict(scenario='test', chassis='mech', seed=1, fire=False, ticks=3,
                   episodes=[dict(start=1, end=1, first_ready=None),
                             dict(start=3, end=3, first_ready=3)])
        row.update(dict.fromkeys(COUNTERS, 3))
        traces = []
        for tick in range(1, 4):
            r = {k: row[k] for k in ('scenario', 'chassis', 'seed', 'fire')}
            r.update(tick=tick, result=dict.fromkeys(COUNTERS, tick))
            traces.append(r)
        return row, traces

    def test_gaps_do_not_inherit_counters(self):
        row, traces = self.fixture()
        result = summarize([row], traces)[0]
        self.assertEqual(result['outside_contact_counters']['replans'], 1)
        self.assertEqual([e['replans'] for e in result['episodes']], [1, 1])
        self.assertEqual(result['episodes'][1]['contact_gap_ticks'], 1)
        self.assertEqual(result['episodes'][1]['regain_opportunity_ticks'], 0)
        self.assertNotIn('replans', row['episodes'][0])

    def test_missing_tick_is_rejected(self):
        row, traces = self.fixture()
        with self.assertRaises(AssertionError): summarize([row], traces[1:])

    def test_totals_must_reconcile(self):
        row, traces = self.fixture()
        row['replans'] += 1
        with self.assertRaises(AssertionError): summarize([row], traces)


if __name__ == '__main__':
    unittest.main()

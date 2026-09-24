#!/usr/bin/env python3
"""Compare frozen direct-pursuit and predictive runs without hiding unmatched outcomes."""
import argparse
import json
import statistics
from pathlib import Path


def indexed(rows):
    result = {}
    for r in rows:
        key = (r['scenario'], r['chassis'], r['seed'], r['fire'])
        assert key not in result, f'duplicate {key}'
        assert r['schema'] == 1
        result[key] = r
    return result


def compare(before, after):
    assert before.keys() == after.keys(), 'matrix mismatch'
    failures, unmatched, excess, lateral_before, lateral_after = [], [], [], [], []
    def fail(key, reason):
        failures.append({'case': key, 'reason': reason})
    for key, a in after.items():
        b = before[key]
        if a['fire']:
            if a['scenario'] == 'intercept_move' and a['outcome'] != 'completed':
                fail(key, 'armed attack-move did not resume/complete')
            continue  # casualties are reported, not compared as equal observation windows
        if a.get('script_rejections', 0):
            fail(key, 'scripted controls rejected in hold fixture')
        if a['scenario'] != 'expiry' and a['outcome'] not in ('window_end', 'completed'):
            fail(key, 'sustained-pursuit failure: ' + a['outcome'])
        scenario = a['scenario']
        if scenario == 'expiry':
            if 'ContactLost' not in a['outcome']:
                fail(key, 'contact expiry missing')
            continue
        if scenario == 'intercept_move':
            if a['outcome'] != 'completed':
                fail(key, 'weapons-hold destination control did not complete')
            continue  # weapons hold deliberately disables attack-move diversion
        if a['first_ready'] is None:
            fail(key, 'no firing opportunity')
        if a['first_ready'] is None or b['first_ready'] is None:
            unmatched.append({'case': key, 'before': b['outcome'], 'after': a['outcome']})
        else:
            if a['first_ready'] > b['first_ready'] + 5:
                fail(key, 'first opportunity regressed by more than five ticks')
            excess.append({'case': key, 'additional_distance': a['approach_distance'] - b['approach_distance']})
            if scenario == 'lateral':
                lateral_before.append(b['first_ready'])
                lateral_after.append(a['first_ready'])
        if scenario == 'distant' and (a['settled_ticks'] < 20 or a['settled_distance'] > .25):
            fail(key, 'stationary settling regression')
        if scenario == 'short_occlusions':
            gaps = [y['start'] - x['end'] - 1 for x, y in zip(a['episodes'], a['episodes'][1:])]
            if sum(0 < g < 30 for g in gaps) < 3:
                fail(key, 'fewer than three short reacquisitions')
    hold_b = [r for r in before.values() if not r['fire']]
    hold_a = [r for r in after.values() if not r['fire']]
    fraction = lambda rows: sum(r['ready_ticks'] for r in rows) / max(1, sum(r['visible_ticks'] for r in rows))
    if fraction(hold_a) < fraction(hold_b) - .02:
        fail('aggregate', 'readiness fraction regressed')
    if excess and statistics.median(r['additional_distance'] for r in excess) > 0:
        fail('aggregate', 'median approach distance increased')
    gain = None
    if lateral_before:
        gain = 1 - statistics.median(lateral_after) / statistics.median(lateral_before)
        if statistics.median(lateral_after) > statistics.median(lateral_before) * .9:
            fail('lateral', 'median improvement below ten percent')
    return {'failures': failures, 'unmatched': unmatched, 'paired_excess_travel': excess,
            'lateral_improvement': gain, 'readiness_before': fraction(hold_b), 'readiness_after': fraction(hold_a)}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument('before', type=Path)
    parser.add_argument('after', type=Path)
    args = parser.parse_args()
    result = compare(indexed(json.loads(args.before.read_text())), indexed(json.loads(args.after.read_text())))
    print(json.dumps(result, indent=2))
    raise SystemExit(bool(result['failures']))

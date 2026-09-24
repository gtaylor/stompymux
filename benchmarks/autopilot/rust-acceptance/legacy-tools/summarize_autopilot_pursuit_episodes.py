#!/usr/bin/env python3
"""Attribute committed pursuit counters to visible-contact episodes without replaying dice.

Input is the ordinary pursuit summary plus its complete JSONL trace files. The
script never supplies data to gameplay decisions. Counters outside visibility
remain separately reported, so a reacquisition cannot inherit hidden travel work.
"""
import argparse
import copy
import json
from pathlib import Path

COUNTERS = ('reversals', 'replans', 'prediction_ticks', 'prediction_fallbacks')


def key(row):
    return tuple(row[k] for k in ('scenario', 'chassis', 'seed', 'fire'))


def summarize(summary, records):
    rows = {key(r): copy.deepcopy(r) for r in summary}
    assert len(rows) == len(summary), 'duplicate cases'
    previous = {}
    for row in rows.values():
        row['outside_contact_counters'] = dict.fromkeys(COUNTERS, 0)
        last = None
        for episode in row['episodes']:
            episode.update(dict.fromkeys(COUNTERS, 0))
            episode['contact_gap_ticks'] = None if last is None else episode['start'] - last - 1
            episode['regain_opportunity_ticks'] = (None if episode['first_ready'] is None
                                                   else episode['first_ready'] - episode['start'])
            last = episode['end']
    for record in records:
        k = key(record)
        row = rows[k]
        result = record['result']
        old_tick, old = previous.get(k, (0, dict.fromkeys(COUNTERS, 0)))
        assert record['tick'] == old_tick + 1, f'missing/duplicate tick for {k}'
        episode = next((e for e in row['episodes'] if e['start'] <= record['tick'] <= e['end']), None)
        target = episode if episode is not None else row['outside_contact_counters']
        for name in COUNTERS:
            change = result[name] - old[name]
            assert change >= 0, f'counter went backwards: {k} {name}'
            target[name] += change
        previous[k] = (record['tick'], {name: result[name] for name in COUNTERS})
    for k, row in rows.items():
        assert previous[k][0] == row['ticks'], f'incomplete trace for {k}'
        for name in COUNTERS:
            assert (sum(e[name] for e in row['episodes']) + row['outside_contact_counters'][name]
                    == row[name]), f'attribution does not reconcile: {k} {name}'
    return list(rows.values())


def records(paths):
    for path in paths:
        with path.open() as source:
            for line in source:
                yield json.loads(line)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('summary', type=Path)
    parser.add_argument('traces', type=Path, nargs='+')
    args = parser.parse_args()
    print(json.dumps(summarize(json.loads(args.summary.read_text()), records(args.traces)), indent=2))

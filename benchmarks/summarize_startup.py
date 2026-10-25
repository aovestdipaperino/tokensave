"""Summarize mcp_startup.py JSONL; bootstrap paired median differences."""
import argparse
import json
import random
import statistics
from pathlib import Path


def summarize(path):
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    result = {'metadata': next(row for row in rows if row['type'] == 'metadata'),
              'latency': {}, 'idle': {}, 'reindex': {}}
    for scenario in ['warm', 'large', 'no-index']:
        selected = [r for r in rows if r['type'] == 'paired' and r['scenario'] == scenario]
        result['latency'][scenario] = {}
        for metric in ['startup_ms', 'first_ms', 'repeated_ms']:
            builds = {label: {r['pair']: r[metric] for r in selected
                      if r['build'] == label and r['pair'] > 0}
                      for label in ['before', 'after']}
            before, after = builds['before'], builds['after']
            if not before or before.keys() != after.keys():
                raise ValueError(f'incomplete paired data: {scenario}')
            differences = [after[i] - before[i] for i in sorted(before)]
            rng = random.Random(20261020)
            bootstrap = sorted(statistics.median(rng.choices(differences, k=len(differences)))
                               for _ in range(10000))
            result['latency'][scenario][metric] = {
                'pairs': len(before),
                'before': statistics.median(before.values()),
                'after': statistics.median(after.values()),
                'paired_delta': statistics.median(differences),
                'paired_delta_bootstrap_95_percent': [bootstrap[250], bootstrap[9750]],
                'pair_zero': {r['build']: r[metric] for r in selected if r['pair'] == 0}}
    for label in ['before', 'after']:
        idle = [r for r in rows if r['type'] == 'idle' and r['build'] == label]
        result['idle'][label] = {}
        for target in [0, 60, 300, 600]:
            sample = [r for r in idle if r.get('target_s', round(r['elapsed_s'] / 60) * 60) == target]
            if sample:
                result['idle'][label][str(target)] = {
                    'instances': len(sample),
                    'median_parent_mib': statistics.median(r['resources']['parent']['rss_mib'] for r in sample),
                    'sum_parent_mib': sum(r['resources']['parent']['rss_mib'] for r in sample),
                    'children': sum(len(r['resources']['children']) for r in sample)}
        cpu = []
        for instance in sorted({r['instance'] for r in idle}):
            samples = sorted((r for r in idle if r['instance'] == instance), key=lambda r: r['elapsed_s'])
            first, last = samples[0], samples[-1]
            cpu.append((last['resources']['parent']['cpu_s'] - first['resources']['parent']['cpu_s'])
                       / (last['elapsed_s'] - first['elapsed_s']) * 100)
        result['idle'][label]['cpu_percent_one_core_per_instance'] = cpu
        rebuild = [r for r in rows if r['type'] == 'reindex' and r['build'] == label]
        result['reindex'][label] = {
            'trials': len(rebuild), 'errors': sum(bool(r['error']) for r in rebuild),
            'median_settled_ms': statistics.median(r['settled_ms'] for r in rebuild),
            'max_peak_mib': {role: max(r['peak_mib'][role] for r in rebuild)
                             for role in ['parent', 'worker', 'other', 'tree']}}
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('measurements', type=Path)
    print(json.dumps(summarize(parser.parse_args().measurements), indent=2))

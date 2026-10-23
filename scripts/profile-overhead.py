#!/usr/bin/env python3
"""Run the existing db_workloads binary in fresh processes in seeded random order.

No privileged cache dropping: all measurements are warm, sequential, one client.
A result is evidence, never an automatic assertion that the 2% target is met.
"""
import argparse
import json
import random
import statistics
import subprocess
from pathlib import Path


def summarize(runs):
    by_mode = {mode: [] for mode in ('off', 'light', 'detailed')}
    for run in runs:
        if not run['passed']:
            raise ValueError('a workload failed correctness validation')
        by_mode[run['config']['profiling']].append(run)
    reference = by_mode['off'][0]
    for run in runs:
        for key in ('files', 'nodes', 'edges'):
            if run['dataset'][key] != reference['dataset'][key]:
                raise ValueError(f'dataset changed: {key}')
        for actual, expected in zip(run['workloads'], reference['workloads']):
            if actual['fingerprints'] != expected['fingerprints']:
                raise ValueError(f"result fingerprints changed: {actual['name']}")
        if [w['name'] for w in run['workloads']] != [w['name'] for w in reference['workloads']]:
            raise ValueError('workload set changed')
    results = []
    for index, workload in enumerate(reference['workloads']):
        distributions = {
            mode: [run['workloads'][index]['p50_us'] for run in group]
            for mode, group in by_mode.items()
        }
        baseline = statistics.median(distributions['off'])
        modes = {}
        for mode, values in distributions.items():
            median = statistics.median(values)
            modes[mode] = {
                'repetitions': len(values), 'p50_us_by_run': values,
                'median_p50_us': median,
                'stdev_p50_us': statistics.stdev(values) if len(values) > 1 else None,
                'overhead_percent': 100 * (median / baseline - 1) if baseline else None,
            }
        results.append({'name': workload['name'], 'modes': modes})
    return {'schema': 'tokensave-profile-overhead/1', 'workloads': results,
            'limitations': ['Warm cache, one sequential client, library API (no MCP transport).',
                            'Detailed currently has the same recording path as light; no sampled traces exist.',
                            'Off vs enabled measures incremental cost, not off vs an uninstrumented build.',
                            'Per-run p50 variation is reported; this is not proof of a universal overhead bound.']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--project', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--repetitions', type=int, default=5)
    parser.add_argument('--iterations', type=int, default=1000)
    parser.add_argument('--seed', type=int, default=754)
    args = parser.parse_args()
    if args.repetitions < 2 or args.iterations < 100:
        parser.error('use at least 2 repetitions and 100 iterations')
    args.out.mkdir(parents=True, exist_ok=True)
    schedule = [(rep, mode) for rep in range(args.repetitions) for mode in ('off', 'light', 'detailed')]
    random.Random(args.seed).shuffle(schedule)
    runs = []
    for rep, mode in schedule:
        output = args.out.resolve() / f'{rep}-{mode}.json'
        subprocess.run([str(args.binary.resolve()), '--project', str(args.project.resolve()),
                        '--profile', mode, '--iterations', str(args.iterations),
                        '--warmup', str(max(10, args.iterations // 10)), '--index-runs', '3',
                        '--out', str(output)], check=True)
        runs.append(json.loads(output.read_text()))
    summary = summarize(runs)
    summary['seed'] = args.seed
    summary['schedule'] = schedule
    (args.out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Paired agent trials with explicit telemetry and independent verification."""
import argparse
import hashlib
import json
import math
import os
import re
from pathlib import Path
import signal
import subprocess
import tempfile
import time


def summarize(events):
    """Count actual provider usage and observed tool activity, never estimates."""
    totals = dict(input_tokens=0, output_tokens=0, cost_usd=0.0,
                  mcp_calls=0, redundant_file_reads=0)
    reads = set()
    usage = 0
    for event in events:
        kind = event['type']
        if kind == 'usage':
            usage += 1
            for key in ('input_tokens', 'output_tokens'):
                value = event[key]
                if type(value) is not int or value < 0:
                    raise ValueError(f'invalid {key}')
                totals[key] += value
            cost = event['cost_usd']
            if type(cost) not in (int, float) or not math.isfinite(cost) or cost < 0:
                raise ValueError('invalid measured cost')
            totals['cost_usd'] += cost
        elif kind == 'mcp_call':
            totals['mcp_calls'] += 1
        elif kind == 'file_read':
            # Adapter supplies canonical path, exact range and content digest.
            # Changed content/ranges are not counted as redundant.
            key = (event['path'], event['range'], event['sha256'])
            totals['redundant_file_reads'] += int(key in reads)
            reads.add(key)
        else:
            raise ValueError(f'unknown event type: {kind}')
    if not usage:
        raise ValueError('provider usage missing; do not substitute estimates')
    return totals


def execute(argv, *, cwd, env, timeout, request=None):
    # Spool telemetry to disk; only a bounded report enters memory. The driver
    # owns native trace retention and must bound its on-disk output.
    with tempfile.TemporaryFile() as capture:
        process = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.PIPE,
            stdout=capture, stderr=subprocess.DEVNULL, start_new_session=True)
        try:
            process.communicate(request.encode() if request is not None else None, timeout=timeout)
            capture.seek(0)
            output = capture.read(16 * 1024 * 1024 + 1)
            if len(output) > 16 * 1024 * 1024:
                raise ValueError('telemetry exceeds 16 MiB; experiment incomplete')
            return process.returncode, output.decode()
        finally:
            # Reap the driver and terminate any remaining descendants in its
            # process group, including on timeout, errors and cancellation.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()


def run(config, output):
    if os.name != 'posix':
        raise ValueError('runner currently requires POSIX process-group cleanup')
    if not re.fullmatch(r'[0-9a-fA-F]{40}', config['commit']):
        raise ValueError('pin a full commit SHA')
    if not Path(config['repository']).is_dir():
        raise ValueError('use a local task repository')
    if type(config.get('repeats', 3)) is not int or config.get('repeats', 3) < 1 or not config['tasks']:
        raise ValueError('at least one repeat and task required')
    for key in ('tokensave_commit', 'agent_version', 'model', 'provider', 'pricing_snapshot'):
        a, b = (config['arms'][arm]['metadata'].get(key) for arm in ('A', 'B'))
        if not a or a == 'PIN_ME' or a != b:
            raise ValueError(f'pin identical {key} in both arms')
    for arm in ('A', 'B'):
        if not Path(config['arms'][arm]['driver'][0]).is_absolute():
            raise ValueError('driver executable must be absolute')
    if output.exists():
        raise ValueError('output already exists; preserve previous trial evidence')
    output.mkdir(parents=True)
    config_bytes = json.dumps(config, sort_keys=True).encode()
    (output / 'manifest.json').write_bytes(config_bytes)
    rows = []
    # Alternate order to reduce warm-cache/time-of-day bias.
    for repeat in range(config.get('repeats', 3)):
        for task in config['tasks']:
            for arm in (['A', 'B'] if repeat % 2 == 0 else ['B', 'A']):
                with tempfile.TemporaryDirectory(prefix='tokensave-caveman-bench-') as temporary:
                    work = Path(temporary)
                    subprocess.run(['git', 'clone', '--quiet', '--no-hardlinks',
                                    config['repository'], str(work / 'repo')], check=True)
                    repo = work / 'repo'
                    subprocess.run(['git', 'checkout', '--quiet', '--detach', config['commit']], cwd=repo, check=True)
                    home = work / 'home'
                    home.mkdir()
                    request = dict(arm=arm, prompt=task['prompt'], repository=str(repo),
                                   home=str(home), metadata=config['arms'][arm]['metadata'])
                    # Driver must scope ALL agent/config/proxy paths to this
                    # disposable home; no real user configuration is modified.
                    env = dict(os.environ, HOME=str(home), USERPROFILE=str(home))
                    start = time.monotonic()
                    try:
                        code, stdout = execute(config['arms'][arm]['driver'],
                            request=json.dumps(request), cwd=repo, env=env,
                            timeout=config.get('timeout_seconds', 600))
                    except subprocess.TimeoutExpired:
                        raise RuntimeError('driver timed out; incomplete experiment, no aggregate claim') from None
                    latency = time.monotonic() - start
                    if code:
                        raise RuntimeError('driver failed; incomplete experiment, no aggregate claim')
                    # Driver stores raw native trace separately and certifies its
                    # capture is complete. No credentials or response bodies here.
                    telemetry = json.loads(stdout)
                    if not re.fullmatch(r'[0-9a-fA-F]{64}', telemetry.get('trace_sha256', '')):
                        raise ValueError('native trace digest missing')
                    if telemetry.get('complete') is not True:
                        raise ValueError('incomplete telemetry')
                    metrics = summarize(telemetry['events'])
                    verdict, _ = execute(task['verify'], cwd=repo, env=env,
                        timeout=config.get('timeout_seconds', 600))
                    row = dict(arm=arm, task=task['id'], repeat=repeat,
                               success=verdict == 0, latency_seconds=latency,
                               **metrics)
                    rows.append(row)
                    with (output / 'trials.jsonl').open('a') as stream:
                        stream.write(json.dumps(row) + '\n')
                    # Retain normalized events to audit aggregate counts.
                    (output / f'{len(rows):04d}-events.json').write_text(json.dumps(telemetry))
    summary = {}
    for arm in ('A', 'B'):
        trials = [row for row in rows if row['arm'] == arm]
        successful = sum(row['success'] for row in trials)
        totals = {key: sum(row[key] for row in trials) for key in
                  ('input_tokens', 'output_tokens', 'mcp_calls', 'redundant_file_reads', 'cost_usd', 'latency_seconds')}
        summary[arm] = dict(trials=len(trials), successful=successful,
            completion_rate=successful / len(trials), **totals,
            cost_per_successful_task=totals['cost_usd'] / successful if successful else None)
    summary['manifest_sha256'] = hashlib.sha256(config_bytes).hexdigest()
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    run(json.loads(args.manifest.read_text()), args.output.resolve())


if __name__ == '__main__':
    main()

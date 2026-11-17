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
            if not math.isfinite(totals['cost_usd']):
                raise ValueError('cost total is not finite')
        elif kind == 'mcp_call':
            if not isinstance(event.get('name'), str) or not event['name'].strip():
                raise ValueError('MCP call name missing')
            totals['mcp_calls'] += 1
        elif kind == 'file_read':
            # Adapter supplies canonical path, exact range and content digest.
            # Changed content/ranges are not counted as redundant.
            path, span, digest = event['path'], event['range'], event['sha256']
            if not isinstance(path, str) or not Path(path).is_absolute():
                raise ValueError('file reads require canonical absolute paths')
            if not isinstance(digest, str) or not re.fullmatch(r'[0-9a-fA-F]{64}', digest):
                raise ValueError('file read SHA-256 missing')
            match = re.fullmatch(r'([1-9][0-9]*):([1-9][0-9]*)', span) if isinstance(span, str) else None
            if span != 'empty' and (not match or int(match[1]) > int(match[2])):
                raise ValueError('file read requires an exact inclusive range')
            key = (path, span, digest.lower())
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


def verify_artifacts(config):
    """Reject changed external scoring inputs before executing or scoring a trial."""
    for name, expected in config.get('verification_artifacts', {}).items():
        path = Path(name)
        if not path.is_absolute() or not re.fullmatch(r'[0-9a-f]{64}', expected):
            raise ValueError('verification artifacts require absolute paths and SHA-256 digests')
        # Scorers are small scripts/fixtures, never model traces or repository trees.
        with path.open('rb') as stream:
            content = stream.read(1024 * 1024 + 1)
        if len(content) > 1024 * 1024 or hashlib.sha256(content).hexdigest() != expected:
            raise ValueError('verification artifact changed; experiment incomplete')


def verify_trace(telemetry, directory):
    if 'trace_file' not in telemetry:
        return  # External adapters may retain native traces independently.
    trace_path = Path(telemetry['trace_file']).resolve()
    if not trace_path.is_relative_to(directory.resolve()):
        raise ValueError('native trace must stay in the private trial directory')
    if trace_path.stat().st_size > 64 * 1024 * 1024:
        raise ValueError('native trace exceeds 64 MiB')
    with trace_path.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    if digest != telemetry['trace_sha256']:
        raise ValueError('native trace hash mismatch')


def run(config, output):
    verify_artifacts(config)
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
    for key in ('model_settings', 'tokensave_sha256', 'driver_sha256', 'toolset', 'experiment',
                'proxy_compression', 'cost_method'):
        values = [config['arms'][arm]['metadata'].get(key) for arm in ('A', 'B')]
        if any(value is not None for value in values) and (not values[0] or values[0] != values[1]):
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
                    trace_directory = output / 'private-traces' / f'{len(rows) + 1:04d}'
                    trace_directory.mkdir(parents=True, mode=0o700)
                    trace_directory.parent.chmod(0o700)
                    request = dict(trace_directory=str(trace_directory), arm=arm, prompt=task['prompt'], repository=str(repo),
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
                    verify_trace(telemetry, trace_directory)
                    metrics = summarize(telemetry['events'])
                    verify_artifacts(config)
                    verdict, _ = execute(task['verify'], cwd=repo, env=env,
                        timeout=config.get('timeout_seconds', 600))
                    verify_artifacts(config)
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

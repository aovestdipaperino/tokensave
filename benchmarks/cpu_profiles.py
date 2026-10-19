"""Linux process-tree CPU/RSS and MCP latency benchmark (stdlib only).

Build the original and candidate with identical Cargo settings, then:
  python3 benchmarks/cpu_profiles.py --baseline /tmp/original/tokensave \
      --candidate target/debug/tokensave --output /tmp/cpu-results.json

CPU is summed across the indexing process and its subprocesses, including
reaped children's CPU time. Peaks use live per-process deltas to avoid
recounting child CPU when workers exit. 100% means one CPU. Peak CPU and aggregate RSS
are sampled at ~100ms; short-lived peaks may be missed. RSS double-counts
shared pages. MCP latency is measured through a separate server/project while
indexing runs, representing contention between sessions (not same-server
request interleaving). Source workloads are deterministic Rust and Python.
Results are descriptive, not CI timing assertions. Use --repo for an existing
representative source tree (copied, with metadata/build outputs omitted).
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import queue
import shutil
import sqlite3
import statistics
import subprocess
import sys
import tempfile
import threading
import time


def process_tree(pid):
    rows = {}
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            fields = (entry / 'stat').read_text().rsplit(')', 1)[1].split()
            rows[int(entry.name)] = (int(fields[1]),
                sum(int(fields[i]) for i in (11, 12)),
                int(fields[21]) * os.sysconf('SC_PAGE_SIZE'))
        except (OSError, ValueError, IndexError):
            pass
    included = {pid}
    while True:
        children = {p for p, r in rows.items() if r[0] in included}
        if children <= included:
            break
        included |= children
    return ({p: rows[p][1] / os.sysconf('SC_CLK_TCK') for p in included if p in rows},
            sum(rows[p][2] for p in included if p in rows))


class MCP:
    def __init__(self, binary, project, env):
        self.log = tempfile.TemporaryFile()
        self.process = subprocess.Popen([binary, 'serve', '--path', str(project)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log, text=True, env=env)
        self.responses = queue.Queue()
        self.counter = 0
        def read():
            for line in self.process.stdout:
                try:
                    self.responses.put(json.loads(line))
                except json.JSONDecodeError:
                    continue
        threading.Thread(target=read, daemon=True).start()
        self.call('initialize', {'protocolVersion': '2024-11-05',
            'capabilities': {}, 'clientInfo': {'name': 'cpu-benchmark', 'version': '1'}})
        self.process.stdin.write(json.dumps({'jsonrpc': '2.0', 'method': 'notifications/initialized'}) + '\n')
        self.process.stdin.flush()
        self.probe()

    def call(self, method, params):
        self.counter += 1
        start = time.perf_counter()
        self.process.stdin.write(json.dumps({'jsonrpc': '2.0', 'id': self.counter,
            'method': method, 'params': params}) + '\n')
        self.process.stdin.flush()
        while True:
            try:
                response = self.responses.get(timeout=30)
            except queue.Empty as error:
                self.log.seek(0)
                raise RuntimeError(f'MCP timed out (exit={self.process.poll()}): '
                    + self.log.read().decode()) from error
            if response.get('id') == self.counter:
                if 'error' in response or response.get('result', {}).get('isError'):
                    raise RuntimeError(response)
                return (time.perf_counter() - start) * 1000

    def probe(self):
        return self.call('tools/call', {'name': 'tokensave_search',
            'arguments': {'query': 'helper', 'limit': 5}})

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        self.log.close()


def graph_signature(project):
    with sqlite3.connect(project / '.tokensave/tokensave.db') as db:
        return {table: hashlib.sha256(json.dumps(db.execute(query).fetchall()).encode()).hexdigest()
            for table, query in {
                'nodes': 'SELECT id,name,kind,file_path,start_line,end_line FROM nodes ORDER BY id',
                'edges': 'SELECT source,target,kind,line FROM edges ORDER BY source,target,kind,line',
                'files': 'SELECT path,content_hash FROM files ORDER BY path',
            }.items()}


def measure(binary, project, env, server, file_count, args, contenders):
    with tempfile.TemporaryFile() as log:
        sentinel = project / '.peer-stop'
        sentinel.unlink(missing_ok=True)
        peer_code = """import hashlib, pathlib, sys, time
stop = pathlib.Path(sys.argv[1]); count = 0; start = time.perf_counter(); data = b'x' * 65536
while not stop.exists():
    for _ in range(50):
        hashlib.sha256(data).digest()
    count += 50
print(count / (time.perf_counter() - start))
"""
        peers = [subprocess.Popen([sys.executable, '-c', peer_code, str(sentinel)],
            env=env, stdout=subprocess.PIPE, text=True) for _ in range(contenders)]
        start = time.perf_counter()
        process = subprocess.Popen([binary, 'sync', str(project), *args],
            stdout=log, stderr=log, env=env)
        samples, latencies = [], []
        previous_cpu, previous_time = {}, start
        finished = threading.Event()
        outcome = {}
        def reap():
            _, status, usage = os.wait4(process.pid, 0)
            outcome.update(elapsed=time.perf_counter() - start,
                cpu=usage.ru_utime + usage.ru_stime,
                code=os.waitstatus_to_exitcode(status))
            process.returncode = outcome['code']
            finished.set()
        waiter = threading.Thread(target=reap, daemon=True)
        waiter.start()
        try:
            while not finished.is_set():
                cpu, rss = process_tree(process.pid)
                now = time.perf_counter()
                delta = sum(max(0, value - previous_cpu.get(pid, 0)) for pid, value in cpu.items())
                samples.append((delta / (now - previous_time) * 100, rss))
                previous_cpu, previous_time = cpu, now
                latencies.append(server.probe())
                finished.wait(0.1)
        finally:
            if not finished.is_set():
                process.terminate()
            waiter.join(timeout=10)
            if not finished.is_set():
                process.kill()
                waiter.join(timeout=10)
            sentinel.touch()
            peer_rates = []
            for peer in peers:
                try:
                    peer_rates.append(float(peer.communicate(timeout=10)[0]))
                except subprocess.TimeoutExpired:
                    peer.kill()
                    peer.wait()
                    raise
            sentinel.unlink(missing_ok=True)
        if not finished.is_set():
            raise RuntimeError('Index process did not exit after termination')
        elapsed = outcome['elapsed']
        if outcome['code']:
            log.seek(0)
            raise RuntimeError(log.read().decode())
    return {'peer_hashes_per_second': sum(peer_rates), 'elapsed_s': elapsed, 'files_per_s': file_count / elapsed,
        'average_cpu_percent': outcome['cpu'] / elapsed * 100,
        'peak_sampled_cpu_percent': max(x[0] for x in samples),
        'peak_aggregate_rss_mib': max(x[1] for x in samples) / 2**20,
        'mcp_requests': len(latencies), 'mcp_median_ms': statistics.median(latencies),
        'mcp_p95_ms': sorted(latencies)[min(len(latencies)-1, int(len(latencies)*.95))]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', required=True)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--repo', type=Path)
    parser.add_argument('--changed-files', type=int, default=16,
        help='source files changed per incremental sync; 0 changes all sources')
    parser.add_argument('--contenders', type=int, default=2)
    parser.add_argument('--files', type=int, default=240)
    parser.add_argument('--functions', type=int, default=80)
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--profile-field', choices=('cpu_usage', 'memory_usage'), default='cpu_usage',
        help='Project resource policy to compare; the other policy is fixed at performance')
    parser.add_argument('--baseline-profile', choices=('low', 'balanced', 'performance'),
        help='Explicit baseline profile for comparing another implementation of the same policy')
    parser.add_argument('--compare-defaults', action='store_true',
        help='Omit both cpu_usage and memory_usage so each build runs its own defaults')
    args = parser.parse_args()
    if min(args.files, args.functions, args.repeats) < 1 or args.contenders < 0 or args.changed_files < 0:
        parser.error('files, functions and repeats must be positive; contenders and changed-files must be nonnegative')
    if platform.system() != 'Linux':
        parser.error('Resource sampler currently requires Linux /proc')
    binaries = {k: str(Path(getattr(args, k)).resolve()) for k in ('baseline', 'candidate')}
    if any(Path(binary).name != 'tokensave' for binary in binaries.values()):
        parser.error('Both binaries must be named tokensave to enable subprocess isolation')
    results = {'host': platform.platform(), 'available_cpus': len(os.sched_getaffinity(0)),
        'workload': vars(args) | {'repo': str(args.repo)}, 'runs': [],
        'binary_sha256': {name: hashlib.sha256(Path(binary).read_bytes()).hexdigest()
            for name, binary in binaries.items()}, 'load_average_at_start': os.getloadavg()}
    Path(args.output).resolve().parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='tokensave-cpu-', dir=Path(args.output).resolve().parent) as temp:
        root = Path(temp)
        env = dict(os.environ, HOME=str(root / 'home'), TOKENSAVE_UPDATE_CHECK='false',
            TOKENSAVE_CLAUDE_HOOKS='false', TOKENSAVE_REPORT_SAVINGS='false',
            TOKENSAVE_SKIP_AGENT_MAINTENANCE='1')
        # Remove tuning overrides so profiles are measured as documented.
        for key in ('TOKENSAVE_WORKERS', 'RAYON_NUM_THREADS', 'TOKENSAVE_DISABLE_SUBPROCESS'):
            env.pop(key, None)
        Path(env['HOME']).mkdir()
        user_config = Path(env['HOME']) / '.tokensave'
        user_config.mkdir()
        (user_config / 'config.toml').write_text('upload_enabled = false\n')
        interactive = root / 'interactive'
        interactive.mkdir()
        (interactive / 'main.rs').write_text('fn helper() {}\nfn main() { helper(); }\n')
        subprocess.run([binaries['baseline'], 'init', str(interactive), '--no-git-hook'],
            env=env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        project = root / 'workload'
        if args.repo:
            shutil.copytree(args.repo, project, ignore=shutil.ignore_patterns(
                '.git', '.tokensave', 'target', 'node_modules', '__pycache__', 'local-only', '.worktrees', 'vendor'))
        else:
            project.mkdir()
            for i in range(args.files):
                if i % 2:
                    body = '\n'.join(f'def f{i}_{j}():\n    return helper() + {j}' for j in range(args.functions))
                    (project / f'm{i}.py').write_text('def helper():\n    return 1\n' + body)
                else:
                    body = '\n'.join(f'pub fn f{i}_{j}() -> u32 {{ helper() + {j} }}' for j in range(args.functions))
                    (project / f'm{i}.rs').write_text('fn helper() -> u32 { 1 }\n' + body)
        subprocess.run([binaries['baseline'], 'init', str(project), '--no-git-hook'],
            env=env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        config_path = project / '.tokensave/config.json'
        config = json.loads(config_path.read_text())
        other_field = 'memory_usage' if args.profile_field == 'cpu_usage' else 'cpu_usage'
        if args.compare_defaults:
            config.pop(other_field, None)
        else:
            config[other_field] = 'performance'
        with sqlite3.connect(project / '.tokensave/tokensave.db') as db:
            file_count = db.execute('SELECT COUNT(*) FROM files').fetchone()[0]
            graph_counts = [db.execute(f'SELECT COUNT(*) FROM {table}').fetchone()[0]
                for table in ('nodes', 'edges')]
        expected_graph = graph_signature(project)
        results['graph_signature'] = expected_graph
        expected_signatures = {}
        results['operation_signatures'] = expected_signatures
        editable_sources = {p: p.read_bytes() for p in project.rglob('*')
            if '.tokensave' not in p.parts and p.suffix in ('.rs', '.py') and p.is_file()}
        if args.changed_files:
            editable_sources = dict(sorted(editable_sources.items())[:args.changed_files])
        results['changed_source_files'] = len(editable_sources)
        results['indexed_files'] = file_count
        results['graph_counts'] = graph_counts
        # Alternate execution order to reduce systematic thermal/cache bias.
        cases = [('baseline', args.baseline_profile)] + [('candidate', p) for p in ('low', 'balanced', 'performance')]
        if args.compare_defaults:
            cases = [('baseline', None), ('candidate', None)]
        for repeat in range(args.repeats):
            for build, profile in (cases if repeat % 2 == 0 else reversed(cases)):
                if profile:
                    config[args.profile_field] = profile
                else:
                    config.pop(args.profile_field, None)
                config_path.write_text(json.dumps(config))
                server = MCP(binaries[build], interactive, env)
                try:
                    for operation, flags in [('full', ['--force']), ('incremental', []), ('unchanged_sync', [])]:
                        if operation == 'incremental':
                            for path, original in editable_sources.items():
                                comment = b'\n// benchmark edit\n' if path.suffix == '.rs' else b'\n# benchmark edit\n'
                                path.write_bytes(original + comment)
                        row = measure(binaries[build], project, env, server, file_count, flags, args.contenders)
                        with sqlite3.connect(project / '.tokensave/tokensave.db') as db:
                            counts = [db.execute(f'SELECT COUNT(*) FROM {table}').fetchone()[0]
                                for table in ('nodes', 'edges')]
                        signature = graph_signature(project)
                        if signature != expected_signatures.setdefault(operation, signature):
                            raise AssertionError(f'Graph differs from original {operation} indexing')
                        if counts != graph_counts:
                            raise AssertionError(f'Graph counts changed: {counts} != {graph_counts}')
                        row.update(build=build, profile=profile or 'original', repeat=repeat, operation=operation)
                        results['runs'].append(row)
                        print(json.dumps(row), flush=True)
                finally:
                    server.close()
                    for path, original in editable_sources.items():
                        path.write_bytes(original)
                Path(args.output).write_text(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()

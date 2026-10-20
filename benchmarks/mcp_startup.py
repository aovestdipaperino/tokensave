"""Linux MCP startup/idle probe (stdlib only). Preserve the tokensave basename.

Use equivalent release binaries in separate directories. Updates, uploads and
agent maintenance are disabled in isolated child user directories. OS page caches
are not flushed. RSS sums double-count shared pages; sampled peaks may be missed.
"""
import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import queue
import shutil
import signal
import sqlite3
import statistics
import subprocess
import tempfile
import threading
import time


def environment(home):
    return dict(os.environ, HOME=str(home), TOKENSAVE_UPDATE_CHECK='0',
                TOKENSAVE_SKIP_AGENT_MAINTENANCE='1')


def snapshot(pid):
    rows = {}
    for path in Path('/proc').iterdir():
        if not path.name.isdigit():
            continue
        try:
            fields = (path / 'stat').read_text().rsplit(')', 1)[1].split()
            rows[int(path.name)] = dict(ppid=int(fields[1]), start=int(fields[19]),
                cpu_s=(int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK'),
                rss_mib=int(fields[21]) * os.sysconf('SC_PAGE_SIZE') / 2**20)
        except (OSError, ValueError, IndexError):
            pass
    included = {pid}
    while True:
        children = {p for p, row in rows.items() if row['ppid'] in included}
        if children <= included:
            break
        included |= children
    children = {}
    for child in included - {pid}:
        try:
            argv = Path(f'/proc/{child}/cmdline').read_bytes().split(b'\0')
            children[child] = dict(rows[child], role='worker' if b'extract-worker' in argv else 'other')
        except OSError:
            pass
    return {'parent': rows.get(pid), 'children': children}


def graph_counts(project):
    connection = sqlite3.connect(f"file:{project / '.tokensave/tokensave.db'}?mode=ro", uri=True)
    try:
        return {table: connection.execute(f'SELECT count(*) FROM {table}').fetchone()[0]
                for table in ['files', 'nodes', 'edges']}
    finally:
        connection.close()


class Server:
    def __init__(self, binary, project, home):
        self.log = tempfile.TemporaryFile()
        self.responses = queue.Queue(maxsize=1024)
        self.counter = 0
        self.stop = threading.Event()
        self.peak = dict(parent=0, worker=0, other=0, tree=0)
        self.seen = {}
        start = time.perf_counter()
        try:
            self.process = subprocess.Popen([str(binary), 'serve', '--path', str(project)],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log, text=True,
                env=environment(home))
        except BaseException:
            self.log.close()
            raise
        self.reader = threading.Thread(target=self.read)
        self.sampler = threading.Thread(target=self.sample)
        try:
            self.reader.start()
            self.sampler.start()
            self.call('initialize', {'protocolVersion': '2024-11-05', 'capabilities': {},
                      'clientInfo': {'name': 'startup-probe', 'version': '1'}})
            self.startup_ms = (time.perf_counter() - start) * 1000
            self.process.stdin.write(json.dumps({'jsonrpc': '2.0',
                'method': 'notifications/initialized'}) + '\n')
            self.process.stdin.flush()
        except BaseException:
            self.close()
            raise

    def read(self):
        for line in self.process.stdout:
            try:
                value = json.loads(line)
            except json.JSONDecodeError:
                value = {'invalid_stdout': line}
            while not self.stop.is_set():
                try:
                    self.responses.put(value, timeout=.1)
                    break
                except queue.Full:
                    pass

    def sample(self):
        while not self.stop.is_set():
            data = snapshot(self.process.pid)
            if data['parent']:
                self.peak['parent'] = max(self.peak['parent'], data['parent']['rss_mib'])
                self.peak['tree'] = max(self.peak['tree'], data['parent']['rss_mib'] + sum(r['rss_mib'] for r in data['children'].values()))
            for role in ['worker', 'other']:
                self.peak[role] = max(self.peak[role], sum(r['rss_mib']
                    for r in data['children'].values() if r['role'] == role))
            self.seen.update({pid: row['start'] for pid, row in data['children'].items()})
            self.stop.wait(.05)

    def call(self, method, params):
        self.counter += 1
        start = time.perf_counter()
        self.process.stdin.write(json.dumps({'jsonrpc': '2.0', 'id': self.counter,
                                            'method': method, 'params': params}) + '\n')
        self.process.stdin.flush()
        deadline = time.monotonic() + 180
        while True:
            response = self.responses.get(timeout=max(.001, deadline - time.monotonic()))
            if 'invalid_stdout' in response:
                raise RuntimeError(response)
            if response.get('id') == self.counter:
                if 'error' in response or response.get('result', {}).get('isError'):
                    raise RuntimeError(response)
                return (time.perf_counter() - start) * 1000

    def query(self, no_index=False):
        return self.call('tools/list', {}) if no_index else self.call('tools/call',
            {'name': 'tokensave_search', 'arguments': {'query': 'helper', 'limit': 5}})

    def diagnostics(self):
        return os.pread(self.log.fileno(), 1_000_000, 0).decode(errors='replace')

    def close(self):
        errors = []
        try:
            try:
                self.process.stdin.close()
            except BrokenPipeError:
                pass
            if self.process.wait(timeout=15):
                errors.append(RuntimeError(self.diagnostics()))
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
            errors.append(RuntimeError('EOF shutdown timed out'))
        finally:
            self.stop.set()
            if self.sampler.ident is not None:
                self.sampler.join()
            # Also check descendants on failed/timed-out shutdown. Stop them
            # before joining the reader in case one inherited the stdout pipe.
            for pid, birth in self.seen.items():
                try:
                    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
                    if int(fields[19]) == birth:
                        os.kill(pid, signal.SIGKILL)
                        errors.append(RuntimeError(f'sampled child {pid} survived shutdown and was stopped'))
                except ProcessLookupError:
                    pass
                except FileNotFoundError:
                    pass
            if self.reader.ident is not None:
                self.reader.join(timeout=2)
            self.process.stdout.close()
            self.log.close()
        if errors:
            raise errors[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before', type=Path, required=True)
    parser.add_argument('--after', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--runs', type=int, default=30)
    parser.add_argument('--idle-seconds', type=int, default=600)
    args = parser.parse_args()
    binaries = {'before': args.before.resolve(), 'after': args.after.resolve()}
    if any(b.stem != 'tokensave' for b in binaries.values()):
        parser.error('keep the tokensave basename: it selects subprocess extraction')
    if args.runs < 2 or args.idle_seconds < 1:
        parser.error('runs must be at least two and idle-seconds positive')
    if 'TOKENSAVE_DISABLE_SUBPROCESS' in os.environ:
        parser.error('unset TOKENSAVE_DISABLE_SUBPROCESS to measure production workers')
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    def save(row):
        with (args.output / 'measurements.jsonl').open('a') as file:
            file.write(json.dumps(row) + '\n')
    if (args.output / 'measurements.jsonl').exists():
        parser.error('use a new output directory; existing measurements are preserved')
    save({'type': 'metadata', 'cpus': os.cpu_count(), 'base': 'bbc0920',
          'affinity': sorted(os.sched_getaffinity(0)), 'platform': platform.platform(),
          'binary_sha256': {k: hashlib.sha256(v.read_bytes()).hexdigest() for k, v in binaries.items()}})
    homes = {}
    for label, binary in binaries.items():
        home = args.output / ('home-' + label)
        home.mkdir()
        homes[label] = home
        subprocess.run([str(binary), 'disable-upload-counter'], env=environment(home),
                       check=True, capture_output=True)
    setup_home = args.output / 'setup-home'
    setup_home.mkdir()
    subprocess.run([str(binaries['before']), 'disable-upload-counter'],
                   env=environment(setup_home), check=True, capture_output=True)
    fixtures = {}
    for name, count in [('warm', 300), ('large', 3000), ('no-index', 0)]:
        project = args.output / name
        project.mkdir()
        fixtures[name] = project
        for i in range(count):
            (project / f'module_{i}.rs').write_text('\n'.join(
                f'pub fn helper_{i}_{j}(x: i32) -> i32 {{ if x > 0 {{ x + {j} }} else {{ {j} }} }}'
                for j in range(20)))
        # A nested Git root prevents the surrounding workspace's ignore rules
        # from suppressing fixtures, and gives both builds the same real HEAD.
        git = ['git', '-C', str(project), '-c', 'core.hooksPath=']
        subprocess.run(['git', '-c', 'init.defaultBranch=main', 'init', '--quiet', str(project)],
                       check=True, capture_output=True, env=environment(setup_home))
        subprocess.run(git + ['add', '.'], check=True, capture_output=True, env=environment(setup_home))
        subprocess.run(git + ['-c', 'user.name=Benchmark', '-c', 'user.email=benchmark@example.invalid',
                       'commit', '--quiet', '--allow-empty', '-m', 'fixture'],
                       check=True, capture_output=True, env=environment(setup_home))
        if count:
            subprocess.run([str(binaries['before']), 'init', '--no-git-hook',
                '--cpu-usage', 'balanced', str(project)], env=environment(setup_home),
                check=True, capture_output=True)
            counts = graph_counts(project)
            if counts['nodes'] != count * 21:
                raise RuntimeError(f'unexpected fixture graph: {counts}')
            save(dict(type='workload', scenario=name, **counts))
    for scenario, project in fixtures.items():
        print('paired', scenario, flush=True)
        for index in range(args.runs):
            order = ['before', 'after'] if index % 2 == 0 else ['after', 'before']
            for label in order:
                server = Server(binaries[label], project, homes[label])
                try:
                    first = server.query(scenario == 'no-index')
                    repeated = [server.query(scenario == 'no-index') for _ in range(10)]
                    save({'type': 'paired', 'scenario': scenario, 'build': label, 'pair': index,
                        'startup_ms': server.startup_ms, 'first_ms': first,
                        'repeated_ms': statistics.median(repeated),
                        'resources': snapshot(server.process.pid), 'peak_mib': dict(server.peak)})
                finally:
                    server.close()
    for index in range(10):
        for label in (['before', 'after'] if index % 2 == 0 else ['after', 'before']):
            project = args.output / 'outdated'
            if project.exists():
                shutil.rmtree(project)
            shutil.copytree(fixtures['warm'], project)
            config_path = project / '.tokensave/config.json'
            config = json.loads(config_path.read_text())
            config.update(root_dir=str(project), last_indexed_version='6.0.0')
            config_path.write_text(json.dumps(config))
            server = Server(binaries[label], project, homes[label])
            try:
                start = time.monotonic()
                first = server.query()
                error = None
                while json.loads(config_path.read_text())['last_indexed_version'] == '6.0.0':
                    if 'version reindex failed:' in server.diagnostics() or time.monotonic() - start > 180:
                        error = server.diagnostics()
                        break
                    time.sleep(.05)
                counts = graph_counts(project)
                if not error and counts['nodes'] != 6300:
                    raise RuntimeError(f'version rebuild changed fixture size: {counts}')
                save({'type': 'reindex', 'build': label, 'trial': index,
                    'startup_ms': server.startup_ms, 'first_ms': first,
                    'settled_ms': (time.monotonic() - start) * 1000,
                    'error': error, 'peak_mib': dict(server.peak)})
            finally:
                server.close()
    servers = []
    try:
        for label in binaries:
            for instance in range(4):
                server = Server(binaries[label], fixtures['warm'], homes[label])
                servers.append((label, instance, server))
                server.query()
        start = time.monotonic()
        targets = sorted({0, args.idle_seconds} | set(range(60, args.idle_seconds + 1, 60)))
        for target in targets:
            time.sleep(max(0, start + target - time.monotonic()))
            for label, instance, server in servers:
                save({'type': 'idle', 'build': label, 'instance': instance,
                    'target_s': target, 'elapsed_s': time.monotonic() - start,
                    'resources': snapshot(server.process.pid)})
            print('idle seconds', target, flush=True)
    finally:
        errors = []
        for _, _, server in servers:
            try:
                server.close()
            except BaseException as error:
                errors.append(error)
        if errors:
            raise errors[0]
    for label, home in homes.items():
        remaining = list((home / '.tokensave/servers').glob('*.json'))
        save({'type': 'cleanup', 'build': label, 'registry_entries': len(remaining)})
        if remaining:
            raise RuntimeError(f'{label}: registry entries survived shutdown')
    print('completed', flush=True)


if __name__ == '__main__':
    main()

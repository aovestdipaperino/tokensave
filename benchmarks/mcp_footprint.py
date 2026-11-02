"""MCP footprint and latency baseline probe (#917). Stdlib only; any OS.

Clones --repo at --rev into a fresh temporary directory, so the server sees no
sibling projects and auto-init skips it, and runs the release binary there with
an isolated home, update checks off and agent maintenance skipped. It records:

* bytes of the `initialize` result, its instructions, and the `tools/list`
  result for the core default and for TOKENSAVE_TOOLS=full;
* token counts for those payloads: chars/4 (tokensave's own estimate) and, when
  the optional `tiktoken` package is importable, its o200k_base and cl100k_base
  encodings;
* cold start: spawn to the `initialize` response and to the first `tools/list`
  response, without and with an index;
* `tokensave init` wall time and the on-disk size of `.tokensave`;
* latency of a fixed query set: `tokensave_context` for every task in
  benchmarks/queries/default.toml and `tokensave_search` for fixed names.

Payloads are written next to the JSON summary. OS file caches are not flushed.
"""
import argparse
import json
import os
from pathlib import Path
import platform
import queue
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import tomllib

SEARCH_QUERIES = ['McpServer', 'handle_tools_list', 'sync', 'Toolset', 'resolve']


def environment(home, toolset=None):
    env = {key: value for key, value in os.environ.items()
           if key not in ('TOKENSAVE_TOOLS', 'TOKENSAVE_REPORT_SAVINGS')}
    env.update(HOME=str(home), USERPROFILE=str(home), TOKENSAVE_UPDATE_CHECK='off',
               TOKENSAVE_SKIP_AGENT_MAINTENANCE='1', TOKENSAVE_AUTO_INIT='0')
    if toolset:
        env['TOKENSAVE_TOOLS'] = toolset
    return env


class Server:
    """One `tokensave serve` process; responses are matched by id."""

    def __init__(self, binary, project, env):
        self.responses = queue.Queue()
        self.counter = 0
        self.start = time.perf_counter()
        self.process = subprocess.Popen(
            [str(binary), 'serve', '--path', str(project)], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env)
        self.reader = threading.Thread(target=self.read, daemon=True)
        self.reader.start()

    def read(self):
        for line in self.process.stdout:
            self.responses.put(line)
        self.responses.put(None)

    def call(self, method, params):
        """Returns (elapsed ms since the request, raw result bytes, result)."""
        self.counter += 1
        sent = time.perf_counter()
        request = {'jsonrpc': '2.0', 'id': self.counter, 'method': method, 'params': params}
        self.process.stdin.write((json.dumps(request) + '\n').encode())
        self.process.stdin.flush()
        deadline = time.monotonic() + 300
        while True:
            line = self.responses.get(timeout=max(.001, deadline - time.monotonic()))
            if line is None:
                raise RuntimeError(f'server exited during {method}')
            response = json.loads(line)
            if response.get('id') != self.counter:
                continue
            if 'error' in response or response.get('result', {}).get('isError'):
                raise RuntimeError(f'{method} failed: {line[:500]!r}')
            result = response['result']
            raw = json.dumps(result, separators=(',', ':'), ensure_ascii=False).encode()
            return (time.perf_counter() - sent) * 1000, raw, result

    def handshake(self):
        _, init_raw, init = self.call('initialize', {
            'protocolVersion': '2025-06-18', 'capabilities': {},
            'clientInfo': {'name': 'footprint-probe', 'version': '1'}})
        init_ms = (time.perf_counter() - self.start) * 1000
        self.process.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
        self.process.stdin.flush()
        _, list_raw, tools = self.call('tools/list', {})
        list_ms = (time.perf_counter() - self.start) * 1000
        return init_ms, list_ms, init_raw, init, list_raw, tools

    def close(self):
        try:
            self.process.stdin.close()
            self.process.wait(timeout=60)
        except (OSError, subprocess.TimeoutExpired):
            self.process.kill()
            self.process.wait()


def tokenizers():
    counters = {'chars/4': lambda text: len(text) // 4}
    try:
        import tiktoken
    except ImportError:
        return counters, None
    for name in ('o200k_base', 'cl100k_base'):
        encoding = tiktoken.get_encoding(name)
        counters[f'tiktoken {name}'] = lambda text, e=encoding: len(e.encode(text))
    return counters, tiktoken.__version__


def summary(values):
    ordered = sorted(values)
    return {'runs': len(ordered), 'median': round(statistics.median(ordered), 1),
            'p95': round(ordered[max(0, round(.95 * len(ordered)) - 1)], 1),
            'min': round(ordered[0], 1), 'max': round(ordered[-1], 1)}


def cold_starts(binary, project, home, runs):
    init, listed = [], []
    for _ in range(runs):
        server = Server(binary, project, environment(home))
        try:
            init_ms, list_ms, *_ = server.handshake()
        finally:
            server.close()
        init.append(init_ms)
        listed.append(list_ms)
    return {'to_initialize_ms': summary(init), 'to_first_tools_list_ms': summary(listed)}


def footprint(binary, project, home, output, counters):
    rows = {}
    for toolset in ('core', 'full'):
        server = Server(binary, project, environment(home, toolset))
        try:
            _, _, init_raw, init, list_raw, tools = server.handshake()
        finally:
            server.close()
        (output / f'initialize-{toolset}.json').write_bytes(init_raw)
        (output / f'tools-list-{toolset}.json').write_bytes(list_raw)
        instructions = init.get('instructions', '')
        texts = {'initialize_result': init_raw.decode(), 'instructions': instructions,
                 'tools_list_result': list_raw.decode()}
        row = {'tools': len(tools['tools']),
               'bytes': {key: len(text.encode()) for key, text in texts.items()},
               'tokens': {name: {key: count(text) for key, text in texts.items()}
                          for name, count in counters.items()}}
        # The quantity guarded by DEFAULT_FOOTPRINT_BYTES in src/mcp/server.rs.
        row['bytes']['tools_list_plus_instructions'] = (
            row['bytes']['tools_list_result'] + row['bytes']['instructions'])
        rows[toolset] = row
    return rows


def query_latency(binary, project, home, runs):
    tasks = tomllib.loads(Path(__file__).with_name('queries').joinpath('default.toml')
                          .read_text(encoding='utf-8'))['query']
    calls = [('tokensave_context', {'task': task['task']}) for task in tasks]
    calls += [('tokensave_search', {'query': name}) for name in SEARCH_QUERIES]
    server = Server(binary, project, environment(home))
    rows = []
    try:
        server.handshake()
        for name, arguments in calls:
            server.call('tools/call', {'name': name, 'arguments': arguments})  # warm-up
            times, size = [], 0
            for _ in range(runs):
                elapsed, raw, _ = server.call('tools/call', {'name': name, 'arguments': arguments})
                times.append(elapsed)
                size = len(raw)
            rows.append({'tool': name, 'arguments': arguments, 'result_bytes': size,
                         'latency_ms': summary(times)})
    finally:
        server.close()
    every = [row['latency_ms']['median'] for row in rows]
    return {'queries': rows, 'median_of_medians_ms': round(statistics.median(every), 1)}


def directory_bytes(path):
    return sum(item.stat().st_size for item in path.rglob('*') if item.is_file())


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--repo', type=Path, required=True, help='git repository to clone')
    parser.add_argument('--rev', required=True, help='commit to check out (pin it)')
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--runs', type=int, default=10, help='cold starts per state')
    parser.add_argument('--query-runs', type=int, default=5, help='timed runs per query')
    args = parser.parse_args()
    binary = args.binary.resolve()
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    counters, tiktoken_version = tokenizers()

    root = Path(tempfile.mkdtemp(prefix='tokensave-footprint-'))
    try:
        project, home = root / 'project', root / 'home'
        home.mkdir()
        subprocess.run(['git', 'clone', '--quiet', '--no-checkout', str(args.repo.resolve()),
                        str(project)], check=True)
        subprocess.run(['git', '-C', str(project), 'checkout', '--quiet', '--detach',
                        args.rev], check=True)
        commit = subprocess.run(['git', '-C', str(project), 'rev-parse', 'HEAD'], check=True,
                                capture_output=True, text=True).stdout.strip()
        version = subprocess.run([str(binary), '--version'], check=True, capture_output=True,
                                 text=True, env=environment(home)).stdout.strip()

        result = {
            'command': [Path(sys.executable).name, *sys.argv],
            'binary_version': version, 'repository_commit': commit,
            'host': {'platform': platform.platform(), 'machine': platform.machine(),
                     'processor': platform.processor(), 'cpus': os.cpu_count(),
                     'python': platform.python_version()},
            'tokenizers': {'chars/4': 'tokensave estimate (src/context/read_modes.rs)',
                           'tiktoken': tiktoken_version},
            'cold_start_without_index': cold_starts(binary, project, home, args.runs),
        }
        started = time.perf_counter()
        subprocess.run([str(binary), 'init', str(project), '--no-git-hook',
                        '--cpu-usage', 'balanced'], check=True, env=environment(home),
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        result['init'] = {'wall_ms': round((time.perf_counter() - started) * 1000, 1),
                          'tokensave_dir_bytes': directory_bytes(project / '.tokensave')}
        result['footprint'] = footprint(binary, project, home, output, counters)
        result['cold_start_with_index'] = cold_starts(binary, project, home, args.runs)
        result['query_latency'] = query_latency(binary, project, home, args.query_runs)
    finally:
        shutil.rmtree(root, ignore_errors=True)
    (output / 'results.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result['footprint'], indent=2))


if __name__ == '__main__':
    main()

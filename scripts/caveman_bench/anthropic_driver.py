#!/usr/bin/env python3
"""Explicitly configured, metered voice-only benchmark agent; no shell tool."""
import argparse
import asyncio
from decimal import Decimal
import hashlib
import json
import math
import os
from pathlib import Path
import re
import sys
import urllib.request

from run import execute
from transport import McpClient, LIMIT

VERSION = 'tokensave-bench-anthropic-v1'
# Keep source-reading MCP surfaces limited to tokensave_read. DB-backed semantic
# results remain unmodified. No hidden shell, delegation, other server or proxy.
TOOLS = ('tokensave_status', 'tokensave_context', 'tokensave_search',
         'tokensave_node', 'tokensave_callers', 'tokensave_callees', 'tokensave_read')
BASE = ('Complete the coding task using the available tools. Start code exploration '
        'with tokensave_context. Preserve exact code, paths, signatures, relationships '
        'and errors. Write requested answer files using write_file. You have no shell. '
        'Independent verification runs after you finish. Use tokensave_read for source; '
        'tokensave_context include_code=true is unsupported in this experiment. '
        'User instructions prevail.')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def driver_digest():
    digest = hashlib.sha256()
    for name in ('anthropic_driver.py', 'transport.py', 'run.py'):
        digest.update(name.encode() + b'\0')
        digest.update(bounded_file(Path(__file__).resolve().parent / name))
    return digest.hexdigest()


def bounded_file(path, limit=LIMIT):
    with path.open('rb') as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError('artifact exceeds size limit')
    return data


def canonical_file(root, name):
    path = (root / name).resolve()
    if not path.is_relative_to(root) or any(part.startswith('.') for part in path.relative_to(root).parts):
        raise ValueError('tool path must stay in visible repository files')
    return path


class Trace:
    def __init__(self, directory):
        self.path = directory / 'agent.jsonl'
        self.stream = self.path.open('xb')
        self.path.chmod(0o600)
        self.size = 0
        self.digest = hashlib.sha256()

    def record(self, kind, payload):
        data = (json.dumps(dict(type=kind, payload=payload), ensure_ascii=False) + '\n').encode()
        if self.size + len(data) > 64 * 1024 * 1024:
            raise ValueError('private trace exceeds 64 MiB; experiment incomplete')
        self.stream.write(data)
        self.stream.flush()
        self.digest.update(data)
        self.size += len(data)

    def close(self):
        self.stream.close()


def usage_event(response, pricing):
    usage = response['usage']
    keys = ('input_tokens', 'output_tokens', 'cache_creation_input_tokens', 'cache_read_input_tokens')
    counts = {}
    for key in keys:
        value = usage.get(key, 0) if key.startswith('cache_') else usage[key]
        if value is None and key.startswith('cache_'):
            value = 0
        if type(value) is not int or value < 0:
            raise ValueError('invalid provider usage')
        counts[key] = value
    # This adapter never enables caching. Refuse unexpected billable categories
    # rather than guessing TTL-specific cache-write prices or surcharges.
    if counts['cache_creation_input_tokens'] or counts['cache_read_input_tokens']:
        raise ValueError('unexpected cached usage; pricing unsupported')
    if any(value for value in (usage.get('server_tool_use') or {}).values()):
        raise ValueError('unexpected server tool billing')
    if usage.get('service_tier') not in (None, 'standard'):
        raise ValueError('nonstandard service tier unsupported')
    if usage.get('inference_geo') not in (None, 'global'):
        raise ValueError('regional pricing unsupported')
    if counts['input_tokens'] > 200000:
        raise ValueError('long-context pricing unsupported')
    if response.get('model') != pricing['model']:
        raise ValueError('returned model differs from pinned pricing model')
    cost = sum(Decimal(counts[key]) * Decimal(str(pricing[key + '_usd_per_million']))
               / Decimal(1_000_000) for key in ('input_tokens', 'output_tokens'))
    return dict(type='usage', input_tokens=counts['input_tokens'],
                output_tokens=counts['output_tokens'], cost_usd=float(cost),
                request_id=response['id'])


def provider_request(payload, key):
    # No retries, redirects, inherited proxy or alternate endpoint. A failed or
    # unmetered request invalidates the experiment rather than hiding billing.
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *_):
            return None
    body = json.dumps(payload, ensure_ascii=False).encode()
    if len(body) > 8 * LIMIT:
        raise ValueError('provider context exceeds 8 MiB')
    request = urllib.request.Request('https://api.anthropic.com/v1/messages', data=body,
        headers={'content-type': 'application/json', 'anthropic-version': '2023-06-01',
                 'x-api-key': key})
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    with opener.open(request, timeout=60) as response:
        content = response.read(LIMIT + 1)
    if len(content) > LIMIT:
        raise ValueError('provider response exceeds 1 MiB')
    return json.loads(content)


def read_event(root, name, body):
    """Hash exact delivered source lines with one shared native/MCP convention."""
    path = canonical_file(root, name)
    lines = body.split('\n') if body else []
    numbers, source = [], []
    for line in lines:
        match = re.fullmatch(r'[ \t]*([1-9][0-9]*)\t(.*)', line)
        if not match:
            raise ValueError('unrecognized or truncated numbered source')
        numbers.append(int(match[1]))
        source.append(match[2])
    if numbers and numbers != list(range(numbers[0], numbers[-1] + 1)):
        raise ValueError('noncontiguous source range')
    return dict(type='file_read', path=str(path),
                range=f'{numbers[0]}:{numbers[-1]}' if numbers else 'empty',
                sha256=sha('\n'.join(source).encode()))


def source_lines(text):
    """Match Rust str::lines, including CRLF and non-newline Unicode controls."""
    if not text:
        return []
    pieces = text.split('\n')
    terminated = len(pieces) - 1
    lines = [line.removesuffix('\r') if i < terminated else line
             for i, line in enumerate(pieces)]
    return lines[:-1] if text.endswith('\n') else lines


def audit_mcp_read(root, args, result):
    if result.get('isError'):
        return []
    content = result.get('content', [])
    if len(content) != 1 or content[0].get('type') != 'text':
        raise ValueError('unsupported read result envelope')
    text = content[0]['text']
    if args.get('format', 'text') == 'json':
        payload = json.loads(text)
    else:
        header, separator, body = text.partition('\n\n')
        payload = dict(line.split(': ', 1) for line in header.strip().splitlines())
        if separator:
            payload['body'] = body
    if payload.get('unchanged') in (True, 'true') or payload.get('mode') in ('map', 'signatures'):
        return []
    if payload.get('mode') not in ('full', 'lines') or 'body' not in payload:
        raise ValueError('unsupported source-read payload')
    return [read_event(root, payload['file'], payload['body'])]


def native_tool(root, name, args):
    path = canonical_file(root, args['path'])
    if name == 'write_file':
        data = args['content'].encode()
        if len(data) > LIMIT:
            raise ValueError('write exceeds 1 MiB')
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return 'written', []
    source = source_lines(bounded_file(path).decode())
    start, end = args.get('start', 1), args.get('end', len(source))
    if type(start) is not int or type(end) is not int or start < 1 or end < start:
        if not source and start == 1 and end == 0:
            return '', [read_event(root, str(path), '')]
        raise ValueError('invalid inclusive line range')
    body = '\n'.join(f'{i:6}\t{line}' for i, line in enumerate(source, 1) if start <= i <= end)
    return body, [read_event(root, str(path), body)]


def validate_metadata(metadata):
    if metadata['agent_version'] != VERSION or metadata['provider'] != 'anthropic':
        raise ValueError('pin the benchmark agent version and anthropic provider')
    if (metadata.get('toolset') != list(TOOLS) or metadata.get('experiment') != 'voice-only'
            or metadata.get('proxy_compression') != 'off'
            or metadata.get('cost_method') != 'usage_times_pinned_prices'):
        raise ValueError('pin the supported voice-only toolset and cost method')
    pricing = metadata['pricing_snapshot']
    if not isinstance(pricing, dict) or pricing.get('model') != metadata['model']:
        raise ValueError('pricing snapshot must identify the pinned model')
    if not pricing.get('source') or not pricing.get('retrieved_at'):
        raise ValueError('pricing source and retrieval date required')
    for key in ('input_tokens_usd_per_million', 'output_tokens_usd_per_million'):
        rate = pricing[key]
        if type(rate) not in (int, float) or not math.isfinite(rate) or rate <= 0:
            raise ValueError('pin positive standard-tier token prices')
    settings = metadata['model_settings']
    if set(settings) != {'max_tokens', 'temperature'}:
        raise ValueError('pin max_tokens and temperature; other model features unsupported')
    if type(settings['max_tokens']) is not int or not 1 <= settings['max_tokens'] <= 8192:
        raise ValueError('max_tokens must be 1..8192')
    if type(settings['temperature']) not in (int, float) or not 0 <= settings['temperature'] <= 1:
        raise ValueError('temperature must be 0..1')
    for key in ('tokensave_sha256', 'driver_sha256'):
        if not re.fullmatch(r'[0-9a-f]{64}', metadata[key]):
            raise ValueError('pin binary and driver SHA-256')
    return pricing, settings


async def agent_loop(mcp, request, system, trace, send):
    catalog = await mcp.call('tools/list', {})
    if catalog.get('nextCursor'):
        raise ValueError('paginated tool catalog unsupported')
    tools = [dict(name=t['name'], description=t.get('description', ''), input_schema=t['inputSchema'])
             for t in catalog['tools'] if t['name'] in TOOLS]
    if {t['name'] for t in tools} != set(TOOLS):
        raise ValueError('pinned MCP toolset unavailable')
    tools += [dict(name=name, description=description, input_schema=schema) for name, description, schema in (
        ('read_file', 'Read visible repository source with inclusive 1-based lines.',
         dict(type='object', properties=dict(path=dict(type='string'), start=dict(type='integer'),
              end=dict(type='integer')), required=['path'])),
        ('write_file', 'Write a complete UTF-8 file inside the repository.',
         dict(type='object', properties=dict(path=dict(type='string'), content=dict(type='string')),
              required=['path', 'content'])))]
    trace.record('tool_catalog', tools)
    metadata = request['metadata']
    pricing, settings = validate_metadata(metadata)
    messages = [dict(role='user', content=request['prompt'])]
    context_bytes = len(json.dumps(dict(system=system, tools=tools, messages=messages)).encode())
    events, identities = [], set()
    root = Path(request['repository']).resolve()
    for turn in range(32):
        payload = dict(model=metadata['model'], service_tier='standard_only',
                       system=system, tools=tools, messages=messages,
                       **settings)
        if context_bytes > 7 * LIMIT:
            raise ValueError('agent context exceeds 7 MiB; experiment incomplete')
        trace.record('provider_request', payload)
        response = send(payload)
        trace.record('provider_response', response)
        event = usage_event(response, pricing)
        if event['request_id'] in identities:
            raise ValueError('duplicate provider request identity')
        identities.add(event['request_id'])
        events.append(event)
        blocks = response['content']
        context_bytes += len(json.dumps(blocks).encode())
        if len(blocks) > 64 or any(b['type'] not in ('text', 'tool_use') for b in blocks):
            raise ValueError('unsupported provider content')
        calls = [block for block in blocks if block['type'] == 'tool_use']
        reason = response['stop_reason']
        if not calls:
            if reason not in ('end_turn', 'max_tokens', 'refusal'):
                raise ValueError('unsupported stop reason')
            return events, reason
        if reason != 'tool_use':
            raise ValueError('tool calls without tool_use stop reason')
        if turn == 31:
            # Do not execute source reads whose results cannot reach another
            # provider request within this trial's fixed turn budget.
            return events, 'turn_limit'
        messages.append(dict(role='assistant', content=blocks))
        results = []
        for block in calls:
            name, args = block['name'], block['input']
            if name not in TOOLS and name not in ('read_file', 'write_file'):
                raise ValueError('unobserved tool surface requested')
            trace.record('tool_call', block)
            if name in TOOLS:
                # The benchmark serves only its disposable graph. Extra graph
                # selection would invalidate the fixture and read audit scope.
                if 'graph_root' in args or 'graph_branch' in args:
                    raise ValueError('cross-project calls unsupported')
                if name == 'tokensave_context' and args.get('include_code'):
                    raise ValueError('context snippets unsupported; use tokensave_read')
                events.append(dict(type='mcp_call', name=name))
                result = await mcp.call('tools/call', dict(name=name, arguments=args))
                if name == 'tokensave_read':
                    events.extend(audit_mcp_read(root, args, result))
                # Exact response JSON enters the provider request; no compression.
                text = json.dumps(result, ensure_ascii=False)
            else:
                text, reads = native_tool(root, name, args)
                events.extend(reads)
            context_bytes += len(json.dumps(text).encode())
            if context_bytes > 7 * LIMIT:
                raise ValueError('agent context exceeds 7 MiB; experiment incomplete')
            trace.record('tool_result', dict(tool_use_id=block['id'], content=text))
            results.append(dict(type='tool_result', tool_use_id=block['id'], content=text))
        messages.append(dict(role='user', content=results))
    # Budget exhaustion is a fully metered task failure, not lost telemetry.
    return events, 'turn_limit'


async def run_driver(request, binary, caveman_source):
    if os.name != 'posix':
        raise ValueError('driver requires POSIX cleanup')
    metadata = request['metadata']
    validate_metadata(metadata)
    if driver_digest() != metadata['driver_sha256']:
        raise ValueError('driver artifact differs from pin')
    # Stream binary hashing; do not retain a release binary in memory.
    with binary.open('rb') as stream:
        binary_digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    if binary_digest != metadata['tokensave_sha256']:
        raise ValueError('TokenSave binary differs from pin')
    root, home = Path(request['repository']).resolve(), Path(request['home']).resolve()
    if not home.is_dir() or any(home.iterdir()) or home == root:
        raise ValueError('driver requires a fresh empty disposable home')
    directory = Path(request['trace_directory']).resolve()
    if directory.is_relative_to(root) or directory.is_relative_to(home):
        raise ValueError('private traces must survive temporary checkout cleanup')
    env = dict(os.environ, HOME=str(home), USERPROFILE=str(home),
        CODEX_HOME=str(home / '.codex'), CLAUDE_CONFIG_DIR=str(home / '.claude'),
        XDG_CONFIG_HOME=str(home / '.config'), TOKENSAVE_SKIP_AGENT_MAINTENANCE='1',
        TOKENSAVE_DISABLE_SUBPROCESS='1', TOKENSAVE_CAVEMAN_COMPATIBILITY='0', TOKENSAVE_TOOLS='full')
    # No inherited TokenSave-owned configuration override may alter an arm.
    env = {k: v for k, v in env.items() if not k.startswith('TOKENSAVE_') or k in (
        'TOKENSAVE_SKIP_AGENT_MAINTENANCE', 'TOKENSAVE_DISABLE_SUBPROCESS',
        'TOKENSAVE_CAVEMAN_COMPATIBILITY', 'TOKENSAVE_TOOLS')}
    system = BASE
    activation = dict(kind='absent')
    if request['arm'] == 'B':
        commit = metadata['caveman_commit']
        if not caveman_source or not re.fullmatch(r'[0-9a-f]{40}', commit):
            raise ValueError('B requires a pinned independent Caveman source checkout')
        code, skill = execute(['git', 'show', commit + ':skills/caveman/SKILL.md'],
            cwd=caveman_source, env=env, timeout=10)
        if (code or len(skill.encode()) > 65536 or 'name: caveman' not in skill
                or sha(skill.encode()) != metadata['caveman_skill_sha256']):
            raise ValueError('pinned Caveman skill unavailable')
        skill_path = home / '.agents' / 'skills' / 'caveman' / 'SKILL.md'
        skill_path.parent.mkdir(parents=True, exist_ok=False)
        skill_path.write_text(skill)
        system += '\n\nApply the independently supplied Caveman skill for this task:\n' + skill
        env['TOKENSAVE_CAVEMAN_COMPATIBILITY'] = '1'
        activation = dict(kind='explicit_benchmark_system_prompt', commit=commit,
                          skill_sha256=sha(skill.encode()))
    elif request['arm'] != 'A':
        raise ValueError('unknown experiment arm')
    key = os.environ.get('ANTHROPIC_API_KEY')
    if not key:
        raise ValueError('ANTHROPIC_API_KEY required for metered trials')
    # The owned MCP process does not need model credentials.
    env.pop('ANTHROPIC_API_KEY', None)
    trace = Trace(directory)
    try:
        trace.record('metadata', metadata)
        trace.record('activation', activation)
        code, _ = execute([str(binary), 'init', str(root), '--no-git-hook'], cwd=root, env=env, timeout=120)
        if code:
            raise ValueError('TokenSave initialization failed')
        async with McpClient([str(binary), 'serve', '--path', str(root)], root, env) as mcp:
            initialized = await mcp.call('initialize', dict(protocolVersion='2024-11-05',
                capabilities={}, clientInfo=dict(name=VERSION, version='1')))
            await mcp.notify('notifications/initialized')
            trace.record('mcp_initialize', initialized)
            system += '\n\n' + initialized.get('instructions', '')
            events, reason = await agent_loop(mcp, request, system, trace,
                                             lambda payload: provider_request(payload, key))
        return dict(complete=True, trace_sha256=trace.digest.hexdigest(), events=events,
                    stop_reason=reason, activation=activation, trace_file=str(trace.path))
    finally:
        trace.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tokensave', required=True, type=Path)
    parser.add_argument('--caveman-source', type=Path)
    args = parser.parse_args()
    try:
        request = json.loads(sys.stdin.buffer.read(LIMIT + 1))
        result = asyncio.run(run_driver(request, args.tokensave.resolve(), args.caveman_source))
        print(json.dumps(result))
    except Exception:
        # Native payloads, API exceptions and environment values may contain
        # secrets. Private traces remain for auditing; stdout stays JSON-only.
        print('Benchmark driver failed; experiment incomplete. Inspect private trial artifacts.',
              file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    main()

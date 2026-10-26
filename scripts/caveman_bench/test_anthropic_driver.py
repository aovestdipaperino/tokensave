import asyncio
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from anthropic_driver import (TOOLS, Trace, VERSION, agent_loop, audit_mcp_read,
                              native_tool, source_lines, usage_event, validate_metadata)
from transport import McpClient


def metadata():
    return dict(agent_version=VERSION, provider='anthropic', model='pinned-test-model',
                toolset=list(TOOLS), experiment='voice-only', proxy_compression='off',
                cost_method='usage_times_pinned_prices',
                model_settings=dict(max_tokens=1024, temperature=0),
                pricing_snapshot=dict(model='pinned-test-model', source='synthetic-test',
                    retrieved_at='2026-10-26', input_tokens_usd_per_million=3,
                    output_tokens_usd_per_million=15),
                tokensave_sha256='a' * 64, driver_sha256='b' * 64)


def response(identifier, content, reason='end_turn'):
    return dict(id=identifier, model='pinned-test-model', content=content, stop_reason=reason,
                usage=dict(input_tokens=100, output_tokens=20))


class UsageTests(unittest.TestCase):
    def test_actual_usage_and_snapshot_prices(self):
        result = usage_event(response('one', []), metadata()['pricing_snapshot'])
        self.assertEqual(result['input_tokens'], 100)
        self.assertEqual(result['output_tokens'], 20)
        self.assertAlmostEqual(result['cost_usd'], .0006)

    def test_missing_invalid_cached_and_server_billing_rejected(self):
        for changes in (dict(input_tokens=True), dict(output_tokens=-1),
                        dict(cache_creation_input_tokens=1), dict(cache_read_input_tokens=1),
                        dict(server_tool_use=dict(web_search_requests=1)),
                        dict(service_tier='priority'), dict(inference_geo='us'),
                        dict(input_tokens=200001)):
            with self.subTest(changes=changes):
                value = response('one', [])
                value['usage'].update(changes)
                with self.assertRaises(ValueError):
                    usage_event(value, metadata()['pricing_snapshot'])
        value = response('one', [])
        del value['usage']['output_tokens']
        with self.assertRaises(KeyError):
            usage_event(value, metadata()['pricing_snapshot'])

    def test_returned_model_must_match_prices(self):
        value = response('one', [])
        value['model'] = 'different-model'
        with self.assertRaises(ValueError):
            usage_event(value, metadata()['pricing_snapshot'])

    def test_invalid_prices_and_model_settings_rejected(self):
        for rate in (0, -1, True, float('nan'), float('inf')):
            value = metadata()
            value['pricing_snapshot']['input_tokens_usd_per_million'] = rate
            with self.assertRaises(ValueError):
                validate_metadata(value)
        value = metadata()
        value['model_settings']['max_tokens'] = True
        with self.assertRaises(ValueError):
            validate_metadata(value)


class ReadTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        (self.root / 'source.py').write_text('first\nsecond\nthird\n')

    def test_native_and_mcp_reads_share_exact_range_and_content_digest(self):
        body, native = native_tool(self.root, 'read_file', dict(path='source.py', start=2, end=3))
        result = dict(content=[dict(type='text', text=json.dumps(dict(
            file='source.py', mode='lines', body=body)))])
        mcp = audit_mcp_read(self.root, dict(format='json'), result)
        self.assertEqual(native, mcp)
        self.assertEqual(native[0]['range'], '2:3')
        self.assertEqual(native[0]['sha256'], hashlib.sha256(b'second\nthird').hexdigest())
        text_result = dict(content=[dict(type='text', text='file: source.py\nmode: lines\n\n' + body)])
        self.assertEqual(audit_mcp_read(self.root, {}, text_result), native)

    def test_maps_stubs_and_errors_are_not_source_reads(self):
        for payload in (dict(file='source.py', mode='map', body='[]'),
                        dict(file='source.py', mode='full', unchanged=True)):
            result = dict(content=[dict(type='text', text=json.dumps(payload))])
            self.assertEqual(audit_mcp_read(self.root, dict(format='json'), result), [])
        self.assertEqual(audit_mcp_read(self.root, {}, dict(isError=True)), [])

    def test_truncation_and_noncontiguous_ranges_invalidate_telemetry(self):
        for body in ('     1\tfirst\n[truncated]', '     1\tfirst\n     3\tthird'):
            result = dict(content=[dict(type='text', text=json.dumps(dict(
                file='source.py', mode='full', body=body)))])
            with self.assertRaises(ValueError):
                audit_mcp_read(self.root, dict(format='json'), result)

    def test_source_line_boundaries_match_tokensave(self):
        self.assertEqual(source_lines('a\r\nb\r'), ['a', 'b\r'])
        self.assertEqual(source_lines('a\vX\u2028Y\n'), ['a\vX\u2028Y'])
        self.assertEqual(source_lines(''), [])
        self.assertEqual(source_lines('\n'), [''])

    def test_native_tools_reject_outside_and_hidden_paths(self):
        for path in ('../outside', '.git/config'):
            with self.assertRaises(ValueError):
                native_tool(self.root, 'write_file', dict(path=path, content='bad'))
        with tempfile.TemporaryDirectory() as outside:
            (self.root / 'linked').symlink_to(Path(outside), target_is_directory=True)
            with self.assertRaises(ValueError):
                native_tool(self.root, 'write_file', dict(path='linked/file', content='bad'))


class FakeMcp:
    def __init__(self):
        self.calls = []

    async def call(self, method, args):
        self.calls.append((method, args))
        if method == 'tools/list':
            return dict(tools=[dict(name=name, description='test', inputSchema=dict(type='object'))
                               for name in TOOLS])
        return dict(content=[dict(type='text', text='exact "signature" and error: unchanged')])


class AgentTests(unittest.IsolatedAsyncioTestCase):
    async def test_all_requests_tools_and_exact_payloads_are_traced(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            trace = Trace(root)
            self.addCleanup(trace.close)
            mcp = FakeMcp()
            replies = iter([
                response('one', [dict(type='tool_use', id='call1', name='tokensave_search',
                                      input=dict(query='invoice'))], 'tool_use'),
                response('two', [dict(type='tool_use', id='call2', name='write_file',
                                      input=dict(path='answer.json', content='{}'))], 'tool_use'),
                response('three', [dict(type='text', text='done')])])
            requests = []

            def send(payload):
                # Freeze request contents just like a network serializer does.
                requests.append(json.loads(json.dumps(payload)))
                return next(replies)

            events, reason = await agent_loop(mcp, dict(repository=str(root), prompt='test',
                metadata=metadata()), 'system', trace, send)
            self.assertEqual(reason, 'end_turn')
            self.assertEqual(requests[0]['service_tier'], 'standard_only')
            self.assertEqual(len([e for e in events if e['type'] == 'usage']), 3)
            self.assertEqual(len([e for e in events if e['type'] == 'mcp_call']), 1)
            self.assertEqual((root / 'answer.json').read_text(), '{}')
            delivered = json.loads(requests[1]['messages'][-1]['content'][0]['content'])
            self.assertEqual(delivered, await mcp.call('tools/call', {}))
            trace.close()
            self.assertEqual(trace.digest.hexdigest(), hashlib.sha256(trace.path.read_bytes()).hexdigest())
            self.assertEqual(trace.path.stat().st_mode & 0o777, 0o600)

    async def test_turn_limit_keeps_all_usage_without_unconsumed_tool_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            trace = Trace(root)
            self.addCleanup(trace.close)
            counter = iter(range(32))

            def send(_):
                return response(str(next(counter)), [dict(type='tool_use', id='call',
                    name='tokensave_status', input={})], 'tool_use')

            events, reason = await agent_loop(FakeMcp(), dict(repository=str(root), prompt='test',
                metadata=metadata()), 'system', trace, send)
            self.assertEqual(reason, 'turn_limit')
            self.assertEqual(sum(e['type'] == 'usage' for e in events), 32)
            self.assertEqual(sum(e['type'] == 'mcp_call' for e in events), 31)

    async def test_duplicate_provider_identity_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            trace = Trace(root)
            self.addCleanup(trace.close)
            value = response('same', [dict(type='tool_use', id='call', name='tokensave_status',
                                           input={})], 'tool_use')
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                await agent_loop(FakeMcp(), dict(repository=str(root), prompt='test',
                    metadata=metadata()), 'system', trace, lambda _: value)


class TransportTests(unittest.IsolatedAsyncioTestCase):
    async def test_stdio_roundtrip_notifications_and_cleanup(self):
        program = '''import json, sys
for line in sys.stdin:
    request = json.loads(line)
    if 'id' not in request:
        continue
    print(json.dumps({'jsonrpc':'2.0','method':'notifications/progress'}), flush=True)
    print(json.dumps({'jsonrpc':'2.0','id':request['id'],'result':request['params']}), flush=True)
'''
        async with McpClient([sys.executable, '-u', '-c', program], None, dict(os.environ)) as client:
            await client.notify('notifications/initialized')
            self.assertEqual(await client.call('echo', dict(exact='value')), dict(exact='value'))
        self.assertIsNotNone(client.process.returncode)

    async def test_timeout_reaps_server(self):
        program = 'import time; time.sleep(30)'
        with self.assertRaises(asyncio.TimeoutError):
            async with McpClient([sys.executable, '-c', program], None, dict(os.environ),
                                 timeout=.05) as client:
                await client.call('echo', {})
        self.assertIsNotNone(client.process.returncode)

class ConfigurationTests(unittest.TestCase):
    def test_manifest_pins_both_arms_and_artifacts_without_mutating_input(self):
        from configure_anthropic import configure
        from run import verify_artifacts
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / 'tokensave'
            binary.write_bytes(b'synthetic binary')
            pricing = root / 'pricing.json'
            pricing.write_text(json.dumps(metadata()['pricing_snapshot']))
            config = dict(repository='/prepared/fixture', commit='a' * 40, tasks=[])
            skill = '---\nname: caveman\n---\nPreserve exact payloads.\n'
            with patch('configure_anthropic.execute', return_value=(0, skill)):
                result = configure(config, binary, 'b' * 40, root, 'c' * 40, pricing)
            self.assertNotIn('arms', config)
            verify_artifacts(result)
            a, b = (result['arms'][arm]['metadata'] for arm in ('A', 'B'))
            for key in ('model', 'model_settings', 'tokensave_sha256', 'driver_sha256', 'toolset'):
                self.assertEqual(a[key], b[key])
            self.assertEqual(b['caveman_skill_sha256'], hashlib.sha256(skill.encode()).hexdigest())
            self.assertEqual(a['caveman'], 'absent')
            pricing.write_text('{}')
            with self.assertRaisesRegex(ValueError, 'artifact changed'):
                verify_artifacts(result)

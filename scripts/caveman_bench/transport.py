"""Bounded, sequential stdio MCP transport owned by a single benchmark trial."""
import asyncio
import json

LIMIT = 1024 * 1024


class McpClient:
    def __init__(self, argv, cwd, env, timeout=30):
        self.argv, self.cwd, self.env, self.timeout = argv, cwd, env, timeout
        self.process = None
        self.sequence = 0

    async def __aenter__(self):
        self.process = await asyncio.create_subprocess_exec(
            *self.argv, cwd=self.cwd, env=self.env, stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL, limit=LIMIT)
        return self

    async def __aexit__(self, *_):
        # EOF permits the server to close its DB normally. Escalate only when
        # it does not stop; the runner also owns the whole trial process group.
        self.process.stdin.close()
        try:
            await asyncio.wait_for(self.process.wait(), 2)
        except asyncio.TimeoutError:
            try:
                self.process.terminate()
            except ProcessLookupError:
                pass
            try:
                await asyncio.wait_for(self.process.wait(), 2)
            except asyncio.TimeoutError:
                self.process.kill()
                await self.process.wait()

    async def send(self, payload):
        data = json.dumps(payload, ensure_ascii=False).encode() + b'\n'
        if len(data) > LIMIT:
            raise ValueError('MCP request exceeds 1 MiB')
        self.process.stdin.write(data)
        await self.process.stdin.drain()

    async def notify(self, method):
        await asyncio.wait_for(self.send(dict(jsonrpc='2.0', method=method)), self.timeout)

    async def call(self, method, params):
        self.sequence += 1
        identifier = self.sequence

        async def exchange():
            await self.send(dict(jsonrpc='2.0', id=identifier, method=method, params=params))
            # Bound notifications, not only individual messages and wall time.
            for _ in range(128):
                line = await self.process.stdout.readline()
                if not line or len(line) > LIMIT:
                    raise ValueError('missing or oversized MCP response')
                response = json.loads(line)
                if response.get('jsonrpc') != '2.0':
                    raise ValueError('invalid MCP response')
                if 'id' not in response:
                    if not isinstance(response.get('method'), str):
                        raise ValueError('invalid MCP notification')
                    continue
                if response['id'] != identifier or 'error' in response:
                    raise ValueError('unexpected MCP response or RPC error')
                return response['result']
            raise ValueError('MCP notification limit exceeded')

        return await asyncio.wait_for(exchange(), self.timeout)

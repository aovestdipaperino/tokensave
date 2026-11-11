"""Linux fixed-workload lifecycle resource check; run from any directory."""
import importlib.util
import json
import os
from pathlib import Path
import resource
import time
import unittest

spec = importlib.util.spec_from_file_location(
    'checks', Path(__file__).resolve().parents[1] / 'tests' / 'test_mcp_cleanup.py')
checks = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checks)
rows = []
for _ in range(20):
    started = time.monotonic()
    cpu = time.process_time()
    result = unittest.TestResult()
    unittest.defaultTestLoader.loadTestsFromModule(checks).run(result)
    if not result.wasSuccessful():
        raise RuntimeError((result.errors, result.failures))
    rows.append({
        'seconds': round(time.monotonic() - started, 3),
        'cpu_seconds': round(time.process_time() - cpu, 3),
        'open_fds': len(os.listdir('/proc/self/fd')),
        'peak_rss_kib': resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
    })
print(json.dumps(rows))

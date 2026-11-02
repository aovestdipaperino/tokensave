import importlib.util
import json
from pathlib import Path
import resource
import statistics
import subprocess
import sys
import time
from unittest import mock

spec = importlib.util.spec_from_file_location('bench', sys.argv[1])
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)
spawn = subprocess.Popen
result = {}
for mode in ['success', 'failed_entry']:
    children = []
    def start(*args, **kwargs):
        if mode == 'success':
            code = 'import sys; sys.stdin.readline(); print(\'{"id":1,"result":{}}\',flush=True); sys.stdin.read()'
        else:
            code = 'import sys,os; sys.stdin.readline(); os.close(1); sys.stdin.read()'
        child = spawn([sys.executable, '-u', '-c', code], **kwargs)
        children.append(child)
        return child
    times, unreaped, open_pipes = [], 0, 0
    for _ in range(20):
        client = bench.TokensaveMcp(Path('unused'))
        begun = time.perf_counter()
        try:
            with mock.patch.object(bench.subprocess, 'Popen', side_effect=start):
                try:
                    with client:
                        pass
                except RuntimeError:
                    if mode != 'failed_entry':
                        raise
            times.append((time.perf_counter() - begun) * 1000)
            child = children[-1]
            unreaped += child.returncode is None
            open_pipes += sum(not stream.closed for stream in (child.stdin, child.stdout))
        finally:
            # Repair old-version leaks after observing them: this measurement
            # never intentionally leaves live children behind.
            child = children[-1]
            child.stdin.close()
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
            child.stdout.close()
    result[mode] = dict(runs=20, median_ms=statistics.median(times),
                        unreaped_children_before_repair=unreaped,
                        open_pipes_before_repair=open_pipes,
                        children_reaped_after_measurement=all(p.returncode is not None for p in children))
result.update(parent_peak_rss_kib=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
              child_peak_rss_kib=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
              workload_temp_bytes=0, workload_allocated_disk_bytes=0, repository_copies=0,
              build_artifacts_bytes=0)
print(json.dumps(result))

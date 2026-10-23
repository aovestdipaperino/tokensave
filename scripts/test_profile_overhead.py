"""Checks the benchmark aggregator refuses incomparable or incorrect runs."""
import copy
import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location('profile_overhead', Path(__file__).with_name('profile-overhead.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def fixtures():
    return [{'passed': True, 'config': {'profiling': mode},
             'dataset': {'files': 1, 'nodes': 3, 'edges': 2},
             'workloads': [{'name': 'lookup', 'p50_us': latency, 'fingerprints': ['same']}]}
            for mode, latency in [('off', 10), ('off', 12), ('light', 12), ('light', 14), ('detailed', 14), ('detailed', 16)]]


class SummaryTests(unittest.TestCase):
    def test_reports_variation_and_relative_cost(self):
        result = module.summarize(fixtures())['workloads'][0]['modes']
        self.assertEqual(result['off']['median_p50_us'], 11)
        self.assertAlmostEqual(result['light']['overhead_percent'], 100 * (13 / 11 - 1))
        self.assertGreater(result['light']['stdev_p50_us'], 0)

    def test_rejects_errors_and_incomparable_results(self):
        for mutate in [lambda r: r.update(passed=False),
                       lambda r: r['dataset'].update(nodes=5),
                       lambda r: r['workloads'][0].update(name='different'),
                       lambda r: r['workloads'][0].update(fingerprints=['changed'])]:
            runs = copy.deepcopy(fixtures())
            mutate(runs[-1])
            with self.assertRaises(ValueError):
                module.summarize(runs)


if __name__ == '__main__':
    unittest.main()

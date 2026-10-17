import copy
import json
from pathlib import Path
from unittest import TestCase
from .flow_integration_report import comparison
from .flow_probe import validate

BASE = Path(__file__).parent


class FlowIntegrationTests(TestCase):
    def inputs(self):
        read = lambda path: json.loads((BASE / path).read_text())
        return (read('fixtures/flow_integration.json'),
                read('observations/flow_integration_viboceros_20261016.json'),
                read('observations/flow_integration_rhino_20261016.json'))

    def test_forward_integration_matches_without_qualifying_the_inverse(self):
        q, a, n = self.inputs()
        for op in q['operations']:
            validate(op)
        report = comparison(q, a, n)
        saved = json.loads((BASE / 'observations/flow_integration_report_20261016.json').read_text())
        self.assertEqual(report, saved)
        self.assertEqual(report['forward_integrals'], 30)
        self.assertEqual(report['matched_forward_integrals'], 30)
        self.assertLess(report['max_relative_forward_length_error'], 1e-12)
        self.assertFalse(report['complete_native_inverse_qualified'])
        self.assertFalse(report['complete_flow_qualified'])
        self.assertFalse(report['native_values_used_as_kernel_inputs'])
        for row in report['results']:
            self.assertGreater(row['coarse_checkpoint_linear_inverse_max_parameter_error'], 1e-3)
            self.assertGreater(row['precise_flow_max_coordinate_error'], 1e-6)
        self.assertEqual([len(r['native_batch_linear_runs']) for r in report['results']], [3, 4, 5])
        for row in report['results']:
            for run in row['native_batch_linear_runs']:
                self.assertEqual(run['closest_checkpoint_fractional_tolerance'], 1e-6)
                self.assertLess(run['normalized_distance_line_residual'], 1e-10)

    def test_native_batch_density_and_scalar_calls_retain_distinct_results(self):
        q, a, n = self.inputs()
        report = comparison(q, a, n)
        for op, result, row in zip(q['operations'], n['results'], report['results']):
            mids = {r['divisions']: r['t'] for r in row['batch_midpoints'] if r['fractional_tolerance'] == 1e-4}
            self.assertGreater(max(mids.values()) - min(mids.values()), 1e-8)
            diag = result['value']['diagnostics']['target']
            count = op['diagnostic_length_samples']
            scalar = diag['length_rows'][count // 2]['length_lookups'][1]['t']
            batch = next(r for r in diag['length_batches'] if r['divisions'] == 2 and r['fractional_tolerance'] == 1e-2)
            self.assertAlmostEqual(scalar, batch['parameters'][1], places=13)

    def test_nonfinite_incomplete_and_mismatched_records_are_rejected(self):
        q, a, n = self.inputs()
        for corruption in ('identity', 'nonfinite', 'nodes', 'prefix', 'batch', 'points'):
            local, native = copy.deepcopy(a), copy.deepcopy(n)
            aa, nn = local['results'][0]['value'], native['results'][0]['value']
            if corruption == 'identity':
                native['results'][0]['id'] = 'wrong'
            elif corruption == 'nonfinite':
                aa['integration_traces']['target'][0]['length'] = float('nan')
            elif corruption == 'nodes':
                aa['integration_traces']['target'][0]['checkpoints'].pop()
            elif corruption == 'prefix':
                nn['diagnostics']['target']['integrals'][0]['prefixes'].pop()
            elif corruption == 'batch':
                nn['diagnostics']['target']['length_batches'].pop()
            else:
                nn['sdk_points'][0][0] = float('inf')
            with self.assertRaises(ValueError, msg=corruption):
                comparison(q, local, native)

    def test_finite_length_disagreement_is_reported(self):
        q, a, n = self.inputs()
        trace = a['results'][0]['value']['integration_traces']['target'][0]
        trace['length'] += .01
        trace['checkpoints'][-1]['length'] = trace['length']
        report = comparison(q, a, n)
        self.assertEqual(report['matched_forward_integrals'], 29)

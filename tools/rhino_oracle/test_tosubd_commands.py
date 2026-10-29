"""Completeness and fail-closed checks for ToSubD document qualification."""
from tools.verify_provenance import reconstruct_measurement, verify_measurement
import copy
import hashlib
import json
import unittest
from .report_tosubd_commands import ROOT, observations, summarize


class ToSubDCommandTests(unittest.TestCase):
    def lines(self):
        report = json.loads((ROOT / 'docs/tosubd-command-qualification.json').read_text())
        return report, ['TOSUBD_' + stage.upper() + ' ' + json.dumps(row)
                        for stage, rows in report['stages'].items() for row in rows]

    def test_coverage_is_complete_and_source_hashes_are_historical(self):
        report, lines = self.lines()
        self.assertEqual(len(observations()[0]), 201)
        self.assertEqual((report['command_cases'], report['getter_cases']), (201, 200))
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(lines)), report)
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(reversed(lines))), report)
        self.assertIs(report['full_native_parity'], False)
        self.assertIs(report['performance_qualified'], False)
        self.assertGreater(verify_measurement(report), 0)

    def test_missing_duplicate_or_unrelated_rows_fail(self):
        _, lines = self.lines()
        for bad in [lines[:-1], lines + lines[:1], [s.replace('tosubd_main_quad_default', 'unrelated') for s in lines]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(bad))

    def test_false_history_selection_geometry_and_numeric_certificates_fail(self):
        report, lines = self.lines()
        for stage, rows in report['stages'].items():
            row = next(r for r in rows if r['mutated'])
            original = 'TOSUBD_' + stage.upper() + ' ' + json.dumps(row)
            for path, bad in [(['grouped_history'], False), (['mutated'], 1),
                              (['check', 'attributes_selection_and_identity'], False),
                              (['check', 'objects', 0, 'qualified'], False),
                              (['check', 'objects', 0, 'faces'], True),
                              (['check', 'objects', 0, 'maximum_control_or_limit_point_error'], float('nan')),
                              (['check', 'objects', 0, 'maximum_sampled_surface_error'], 1.1e-6)]:
                changed = copy.deepcopy(row)
                target = changed
                for key in path[:-1]:
                    target = target[key]
                target[path[-1]] = bad
                with self.assertRaises(ValueError, msg=(stage, path)):
                    summarize('\n'.join('TOSUBD_' + stage.upper() + ' ' + json.dumps(changed) if s == original else s for s in lines))


if __name__ == '__main__':
    unittest.main()

import copy
import json
import unittest
from .report_soft_crease_session import ROOT, summarize


class SoftCreaseSessionReportTests(unittest.TestCase):
    def report(self):
        return json.loads((ROOT / 'docs/subd-crease-session-qualification.json').read_text())

    @staticmethod
    def lines(report):
        return [prefix + ' ' + json.dumps(row) for prefix, rows in report['stages'].items() for row in rows]

    def test_complete_current_report(self):
        report = self.report()
        self.assertEqual(report, summarize('\n'.join(self.lines(report))))
        self.assertEqual((report['profile_objects'], report['application_workflows']), (50, 96))

    def test_incomplete_duplicate_and_failed_replays_cannot_qualify(self):
        report = self.report()
        lines = self.lines(report)
        for bad in [lines[:-1], lines + lines[-1:]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(bad))
        for field, value in [('qualified', False), ('maximum_sampled_surface_error', float('nan')),
                             ('maximum_control_or_limit_point_error', 1.01e-6)]:
            bad = copy.deepcopy(report)
            bad['stages']['SOFT_SESSION'][0]['check'][field] = value
            with self.assertRaises(ValueError):
                summarize('\n'.join(self.lines(bad)))
        bad = copy.deepcopy(report)
        bad['stages']['SOFT_UI'][0]['preselected'] = 1
        with self.assertRaises(ValueError):
            summarize('\n'.join(self.lines(bad)))

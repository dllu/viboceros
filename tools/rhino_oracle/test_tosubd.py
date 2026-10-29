"""Immutable conversion evidence and fail-closed kernel qualification."""
from tools.verify_provenance import reconstruct_measurement, verify_measurement
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import tosubd_cases as cases
from .report_tosubd import ROOT, HERE, FAMILIES, EXPLORATORY, observations, targets, summarize
from .structural_sdk import stage_helpers, validate_operations


class ToSubDTests(unittest.TestCase):
    def test_fixed_recipes_archived_sources_and_native_lifecycle(self):
        flags = dict(tosubd_discovery='discovery', tosubd_surface_details='surface_details',
                     tosubd_surface_edges='surface_edges', tosubd_features='features', tosubd_poles='poles')
        for family, count in FAMILIES + EXPLORATORY:
            query = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
            self.assertEqual(query, cases.request(**({flags[family]: True} if family in flags else {})))
            validate_operations(query)
            native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
            self.assertEqual(len(native['results']), count)
            self.assertEqual([r['id'] for r in native['results']], [r['id'] for r in query['operations']])
            self.assertEqual(native['provenance']['request_sha256'], hashlib.sha256((HERE / 'fixtures' / (family + '.json')).read_bytes()).hexdigest())
            for row in native['results']:
                self.assertEqual(row['value']['input'], cases.all_cases()[row['id']])
            for name, sha in native['provenance']['sources'].items():
                data = (HERE / name).read_bytes()
                if hashlib.sha256(data).hexdigest() != sha:
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), sha, name)
        values, engine = observations()
        self.assertEqual((len(values), engine), (139, '8.32.26160.13001'))
        expected = targets(values)
        self.assertEqual([len(v) for v in expected.values()], [16, 45, 63])
        self.assertEqual(sum(v is None for stage in expected.values() for v in stage.values()), 1)
        self.assertIsNone(expected['surface_control_kernel']['tosubd_poles_cusp_0', 0])

    def test_strict_operations_and_isolated_helper_imports(self):
        query = cases.request()
        for edit in [dict(case='arbitrary'), dict(extra=True), dict(id='different'), dict(case=[]), dict(op='eval')]:
            bad = copy.deepcopy(query)
            bad['operations'][0].update(edit)
            with self.assertRaises(ValueError):
                cases.validate(bad['operations'][0])
            if 'op' not in edit:
                with self.assertRaises(ValueError):
                    validate_operations(bad)
        modules = ['tosubd_extrusion_cases', 'tosubd_cases', 'tosubd_probe', 'tosubd_info', 'subd_box_probe', 'join_probe', 'merge_edges_probe']
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(query, HERE, Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()}, {m + '.py' for m in modules})
            subprocess.run([sys.executable, '-I', '-c', 'import sys;sys.path.insert(0,sys.argv[1]);import ' + ','.join(modules), directory], check=True, capture_output=True)

    def lines(self):
        report = json.loads((ROOT / 'docs/tosubd-kernel-qualification.json').read_text())
        return report, ['TOSUBD_' + stage.upper() + ' ' + json.dumps(row) for stage, rows in report['stages'].items() for row in rows]

    def test_complete_report_is_order_independent_and_historical(self):
        report, lines = self.lines()
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(lines)), report)
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(reversed(lines))), report)
        self.assertEqual(report['kernel_comparisons'], 124)
        for name in ['command_qualified', 'getter_qualified', 'full_native_parity', 'performance_qualified']:
            self.assertIs(report[name], False)
        self.assertGreater(verify_measurement(report), 0)

    def test_missing_duplicate_or_invalid_comparisons_cannot_qualify(self):
        report, lines = self.lines()
        for data in [lines[:-1], lines + lines[-1:]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(data))
        for stage, rows in report['stages'].items():
            row = next(r for r in rows if 'check' in r)
            original = 'TOSUBD_' + stage.upper() + ' ' + json.dumps(row)
            for key, bad in [('qualified', False), ('vertices', True), ('faces', 0),
                             ('maximum_control_or_limit_point_error', float('nan')),
                             ('maximum_sampled_surface_error', 1.1e-6),
                             ('surface_facets_correspond', False), ('compared_surface_faces', True)]:
                changed = copy.deepcopy(row)
                changed['check'][key] = bad
                data = [s if s != original else 'TOSUBD_' + stage.upper() + ' ' + json.dumps(changed) for s in lines]
                with self.assertRaises(ValueError, msg=(stage, key)):
                    summarize('\n'.join(data))
            changed = dict(row, source=True)
            with self.assertRaises(ValueError):
                summarize('\n'.join(s if s != original else 'TOSUBD_' + stage.upper() + ' ' + json.dumps(changed) for s in lines))
        rejected = next(r for r in report['stages']['surface_control_kernel'] if r.get('rejected'))
        original = 'TOSUBD_SURFACE_CONTROL_KERNEL ' + json.dumps(rejected)
        changed = dict(rejected, rejected=False)
        with self.assertRaises(ValueError):
            summarize('\n'.join(s if s != original else 'TOSUBD_SURFACE_CONTROL_KERNEL ' + json.dumps(changed) for s in lines))


if __name__ == '__main__':
    unittest.main()

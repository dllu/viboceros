"""Immutable ellipsoid evidence and fail-closed qualification controls."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import subd_ellipsoid_probe as probe
from .report_subd_ellipsoid import ROOT, HERE, FAMILIES, EXPLORATORY, summarize, native_history_error
from .structural_sdk import stage_helpers, validate_operations


class SubDEllipsoidTests(unittest.TestCase):
    def test_fixed_inputs_lifecycle_and_archived_producers(self):
        for family, count in FAMILIES + EXPLORATORY:
            path = HERE / 'fixtures' / (family + '.json')
            request = json.loads(path.read_text())
            if family != 'subd_ellipsoid_detail_diagnostics':
                current = probe.request(discovery=family.endswith('discovery'), options=family.endswith('options'), shapes=family.endswith('shapes'), details=family.endswith('details'))
                self.assertEqual(request, current)
            validate_operations(request)
            native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
            self.assertEqual(len(native['results']), count)
            self.assertEqual([r['id'] for r in native['results']], [o['id'] for o in request['operations']])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), native['provenance']['request_sha256'])
            for name, sha in native['provenance']['sources'].items():
                data = (HERE / name).read_bytes()
                if hashlib.sha256(data).hexdigest() != sha:
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), sha, name)
            if (family, count) in FAMILIES:
                for row in native['results']:
                    v = row['value']
                    cases = probe.DETAILS if family.endswith('details') else probe.SCENES
                    self.assertEqual(v['input'], cases[row['id']])
                    self.assertNotIn('Unknown command:', v['history'])
                    self.assertEqual(sum(e['name'] == 'SubDEllipsoid' for e in v['events']), 1)
                    self.assertLessEqual(native_history_error(v['after_script'], v['redo']), 1e-6)

    def test_native_history_checks_data_exactly_and_bounds_evaluation_roundoff(self):
        rows = json.loads((HERE / 'observations/subd_ellipsoid.json').read_text())['results']
        v = next(r['value'] for r in rows if r['id'] == 'subd_ellipsoid_four_tri')
        self.assertLessEqual(native_history_error(v['after_script'], v['redo']), 1e-6)
        for field in ['control', 'sample', 'face', 'crc']:
            after = copy.deepcopy(v['redo'])
            if field == 'control':
                after[0]['subd']['points'][0][0] += 1e-12
            elif field == 'sample':
                after[0]['subd']['surface_points'][0][0] += 1.1e-6
            elif field == 'face':
                after[0]['subd']['surface_mesh']['faces'][0][0] += 1
            else:
                after[0]['crc'] += 1
            with self.assertRaises(ValueError, msg=field):
                native_history_error(v['after_script'], after)

    def test_isolated_helper_staging_and_fixed_recipe_validation(self):
        request = probe.request()
        modules = ['subd_ellipsoid_probe', 'subd_box_probe', 'join_probe', 'merge_edges_probe']
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            stage_helpers(request, HERE, job)
            self.assertEqual({p.name for p in job.iterdir()}, {m + '.py' for m in modules})
            subprocess.run([sys.executable, '-I', '-c', 'import sys;sys.path.insert(0,sys.argv[1]);import ' + ','.join(modules), str(job)], check=True, capture_output=True, text=True)
        for edit in [dict(case='arbitrary'), dict(extra=True), dict(id='different'), dict(case=[]), dict(op='eval')]:
            bad = copy.deepcopy(request)
            bad['operations'][0].update(edit)
            with self.assertRaises(ValueError):
                probe.validate(bad['operations'][0])
            if 'op' not in edit:
                with self.assertRaises(ValueError):
                    validate_operations(bad)

    def lines(self):
        report = json.loads((ROOT / 'docs/subd-ellipsoid-qualification.json').read_text())
        return report, ['SUBD_ELLIPSOID_' + stage.upper() + ' ' + json.dumps(row) for stage, rows in report['stages'].items() for row in rows]

    def test_complete_report_and_parallel_output_order(self):
        report, lines = self.lines()
        self.assertEqual(report, summarize('\n'.join(lines)))
        self.assertEqual(report, summarize('\n'.join(reversed(lines))))
        self.assertEqual((report['command_cases'], report['getter_cases'], report['successful_constructions']), (54, 64, 53))
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        self.assertEqual(len(report['exploratory_recipes']), 61)
        for path, sha in report['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), sha, path)

    def test_missing_failed_or_oversized_checks_cannot_qualify(self):
        report, lines = self.lines()
        for data in [lines[:-1], lines + lines[-1:]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(data))
        for key, bad in [('qualified', False), ('maximum_control_or_limit_point_error', True), ('maximum_sampled_surface_error', float('nan')), ('maximum_sampled_surface_error', 1.1e-6), ('faces', 0), ('compared_surface_faces', True), ('surface_facets_correspond', False), ('common_facet_triangulation', False)]:
            row = copy.deepcopy(report['stages']['command'][0])
            row['check']['objects'][0][key] = bad
            modified = lines[:]
            modified[0] = 'SUBD_ELLIPSOID_COMMAND ' + json.dumps(row)
            with self.assertRaises(ValueError, msg=key):
                summarize('\n'.join(modified))

    def test_declined_admission_and_getter_only_retry_are_explicit(self):
        report, lines = self.lines()
        self.assertNotIn('subd_ellipsoid_retry_second', {r['id'] for r in report['stages']['command']})
        self.assertIn('subd_ellipsoid_retry_second', {r['id'] for r in report['stages']['getter']})
        for index, line in enumerate(lines):
            row = json.loads(line.split(' ', 1)[1])
            if line.startswith('SUBD_ELLIPSOID_COMMAND ') and row.get('declined'):
                row['declined'] = False
                bad = lines[:]
                bad[index] = 'SUBD_ELLIPSOID_COMMAND ' + json.dumps(row)
                with self.assertRaises(ValueError):
                    summarize('\n'.join(bad))


if __name__ == '__main__':
    unittest.main()

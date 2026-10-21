"""Immutable subdivision captures and fail-closed report validation."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from . import subdivide_probe as probe
from .report_subdivide import ROOT, HERE, FAMILIES, EXPLORATORY, getter_eligible, observations, summarize
from .structural_sdk import stage_helpers, validate_operations


class SubdivisionTests(unittest.TestCase):
    def test_fixed_recipes_lifecycle_and_archived_sources(self):
        for family, count in FAMILIES + EXPLORATORY:
            request = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
            flags = {name: family == 'subdivide_' + suffix for name, suffix in
                     [('discovery', 'discovery'), ('details', 'details'), ('orientations', 'orientations'), ('extra', 'extra'), ('ngons', 'ngons')]}
            self.assertEqual(request, probe.request(**flags))
            validate_operations(request)
            native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
            self.assertEqual(len(native['results']), count)
            self.assertEqual([r['id'] for r in native['results']], [o['id'] for o in request['operations']])
            self.assertEqual(hashlib.sha256((HERE / 'fixtures' / (family + '.json')).read_bytes()).hexdigest(), native['provenance']['request_sha256'])
            for row in native['results']:
                self.assertEqual(row['value']['input'], probe.all_cases()[row['id']])
            for name, sha in native['provenance']['sources'].items():
                data = (HERE / name).read_bytes()
                if hashlib.sha256(data).hexdigest() != sha:
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), sha, name)
        rows, engine = observations()
        self.assertEqual(engine, '8.32.26160.13001')
        self.assertEqual(len(rows), 141)
        self.assertEqual(sum(getter_eligible(v) for v in rows.values()), 135)
        self.assertEqual(sum(bool(v['input'].get('cancel')) for v in rows.values()), 2)

    def test_diagnostic_rejected_options_remain_excluded(self):
        rows = json.loads((HERE / 'observations/subdivide_discovery.json').read_text())['results']
        rejected = {r['id'] for r in rows if any('Unknown command:' in s['history'] for s in r['value']['steps'])}
        self.assertTrue({'divide_subd_keep', 'divide_mesh_keep', 'divide_subd_number', 'divide_mesh_number'} <= rejected)
        self.assertFalse(set(observations()[0]) & {r['id'] for r in rows})

    def test_strict_recipe_validation_and_isolated_helper_imports(self):
        request = probe.request()
        for edit in [dict(case='arbitrary'), dict(extra=True), dict(id='different'), dict(case=[]), dict(op='eval')]:
            bad = copy.deepcopy(request)
            bad['operations'][0].update(edit)
            with self.assertRaises(ValueError):
                probe.validate(bad['operations'][0])
            if 'op' not in edit:
                with self.assertRaises(ValueError):
                    validate_operations(bad)
        modules = ['subdivide_probe', 'subd_box_probe', 'join_probe', 'merge_edges_probe']
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(request, HERE, Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()}, {m + '.py' for m in modules})
            subprocess.run([sys.executable, '-I', '-c', 'import sys;sys.path.insert(0,sys.argv[1]);import ' + ','.join(modules), directory], check=True, capture_output=True)

    def report_lines(self):
        report = json.loads((ROOT / 'docs/subdivide-qualification.json').read_text())
        lines = ['SUBDIVIDE_' + stage.upper() + ' ' + json.dumps(row) for stage, rows in report['stages'].items() for row in rows]
        return report, lines

    def test_complete_report_is_order_independent_and_current(self):
        report, lines = self.report_lines()
        self.assertEqual(summarize('\n'.join(lines)), report)
        self.assertEqual(summarize('\n'.join(reversed(lines))), report)
        self.assertEqual((report['captured_workflows'], report['command_cases'], report['getter_cases']), (141, 139, 135))
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])
        for path, sha in report['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), sha, path)

    def test_missing_duplicate_or_invalid_comparisons_cannot_qualify(self):
        report, lines = self.report_lines()
        for data in [lines[:-1], lines + lines[-1:]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(data))
        for stage in ['command', 'getter', 'mesh_kernel']:
            row = next(r for r in report['stages'][stage] if stage == 'mesh_kernel' or 'maximum_vertex_error' in r['check']['objects'][0])
            for key, bad in [('qualified', False), ('maximum_vertex_error', True), ('maximum_vertex_error', float('nan')), ('maximum_vertex_error', 1.1e-6), ('faces', 0), ('stored_pieces_correspond', False), ('triangle_facets_correspond', False)]:
                changed = copy.deepcopy(row)
                check = changed['check'] if stage == 'mesh_kernel' else changed['check']['objects'][0]
                check[key] = bad
                original = 'SUBDIVIDE_' + stage.upper() + ' ' + json.dumps(row)
                modified = ['SUBDIVIDE_' + stage.upper() + ' ' + json.dumps(changed) if s == original else s for s in lines]
                with self.assertRaises(ValueError, msg=(stage, key)):
                    summarize('\n'.join(modified))
        row = report['stages']['command'][0]
        for key, bad in [('maximum_control_or_limit_point_error', True), ('maximum_sampled_surface_error', 1.1e-6), ('surface_facets_correspond', False), ('compared_surface_faces', True)]:
            changed = copy.deepcopy(row)
            changed['check']['objects'][0][key] = bad
            original = 'SUBDIVIDE_COMMAND ' + json.dumps(row)
            with self.assertRaises(ValueError, msg=key):
                summarize('\n'.join('SUBDIVIDE_COMMAND ' + json.dumps(changed) if s == original else s for s in lines))


if __name__ == '__main__':
    unittest.main()

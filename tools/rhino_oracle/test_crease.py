"""Immutable crease recipes, public helpers and fail-closed certificates."""
import base64
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from . import crease_probe as probe
from .report_crease import DIAGNOSTICS, FAMILIES, HERE, ROOT, observations, summarize
from .structural_sdk import stage_helpers, validate_operations


class CreaseTests(unittest.TestCase):
    def test_fixed_recipes_and_immutable_capture_sources(self):
        for family in [f for f, _, _ in FAMILIES] + DIAGNOSTICS:
            path = HERE / 'fixtures' / (family + '.json')
            query = json.loads(path.read_text())
            flag = {'crease_main_v3': 'main', 'crease_extra': 'extra',
                    'crease_policy': 'policy', 'crease_mixed': 'mixed',
                    'crease_overlap': 'overlap'}.get(family)
            if family in [f for f, _, _ in FAMILIES] + ['crease_discovery']:
                self.assertEqual(query, probe.request(**({flag: True} if flag else {})))
            validate_operations(query)
            native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), native['provenance']['request_sha256'])
            for name, sha in native['provenance']['sources'].items():
                data = (HERE / name).read_bytes()
                if hashlib.sha256(data).hexdigest() != sha:
                    data = gzip.decompress((HERE / 'observations/source_snapshots' / (sha + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(), sha, name)
            for row in native.get('results', []):
                self.assertEqual(row['value']['input'], probe.ALL_CASES[row['id']])
                if row['value'].get('source_3dm'):
                    data = (HERE / 'fixtures' / family / (row['id'] + '.3dm')).read_bytes()
                    self.assertEqual(data, base64.b64decode(row['value']['source_3dm'], validate=True))

    def test_diagnostics_stay_outside_successful_qualification(self):
        expected, engine = observations()
        self.assertEqual(engine, '8.32.26160.13001')
        self.assertEqual(len(expected), 110)
        for family in DIAGNOSTICS:
            native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
            if family == 'crease_discovery':
                self.assertEqual(len(native['results']), 30)
                self.assertFalse(set(expected) & {r['id'] for r in native['results']})
            else:
                self.assertIs(native['excluded_from_qualification'], True)
                self.assertIn('error', native)
        rows = json.loads((HERE / 'observations/crease_main_v3.json').read_text())['results']
        rejected = [r for r in rows if r['value']['success'] is False]
        self.assertEqual(len(rejected), 8)
        self.assertTrue(all(r['value']['input'].get('postselect') for r in rejected))
        self.assertFalse(set(expected) & {r['id'] for r in rejected})

    def test_strict_recipes_and_standalone_public_helper_imports(self):
        query = probe.request()
        for change in [dict(case='arbitrary'), dict(extra=True), dict(id='different'),
                       dict(case=[]), dict(op='eval')]:
            bad = copy.deepcopy(query)
            bad['operations'][0].update(change)
            with self.assertRaises(ValueError):
                probe.validate(bad['operations'][0])
            if 'op' not in change:
                with self.assertRaises(ValueError):
                    validate_operations(bad)
        modules = ['crease_probe', 'crease_cases', 'subdivide_probe',
                   'subd_box_probe', 'join_probe', 'merge_edges_probe']
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(query, HERE, Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()}, {m + '.py' for m in modules})
            subprocess.run([sys.executable, '-I', '-c',
                            'import sys;sys.path.insert(0,sys.argv[1]);import ' + ','.join(modules),
                            directory], check=True, capture_output=True)

    def report_lines(self):
        report = json.loads((ROOT / 'docs/crease-qualification.json').read_text())
        lines = ['CREASE_COMMAND ' + json.dumps(row) for row in report['cases']]
        lines.extend('CREASE_GETTER ' + json.dumps(row) for rows in report['getters'].values() for row in rows)
        return report, lines

    def test_report_reconstructs_and_uses_current_sources(self):
        report, lines = self.report_lines()
        self.assertEqual(summarize('\n'.join(lines)), report)
        self.assertEqual(summarize('\n'.join(reversed(lines))), report)
        self.assertEqual(report['command_cases'], 110)
        self.assertEqual(report['captured_workflows'], 118)
        self.assertEqual(report['getter_cases'], 220)
        self.assertEqual({phase: len(rows) for phase, rows in report['getters'].items()},
                         {'preselected': 110, 'postselected': 110})
        self.assertFalse(report['full_native_parity'])
        self.assertFalse(report['performance_qualified'])

    def test_incomplete_or_invalid_comparisons_cannot_qualify(self):
        report, lines = self.report_lines()
        for data in [lines[:-1], lines + lines[-1:]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(data))
        row = report['cases'][0]
        for key, bad in [('maximum_error', True), ('maximum_error', float('nan')),
                         ('maximum_error', float('inf')), ('maximum_error', -1.),
                         ('maximum_error', 1.1e-6), ('identity', False),
                         ('attributes', 1), ('history', False), ('processed_objects', True),
                         ('processed_objects', 0), ('native_empty_ngon_slots', 1),
                         ('id', 'unknown')]:
            changed = dict(row, **{key: bad})
            with self.assertRaises(ValueError, msg=(key, bad)):
                summarize('\n'.join(['CREASE_COMMAND ' + json.dumps(changed)] + lines[1:]))
        for key in row:
            changed = dict(row)
            del changed[key]
            with self.assertRaises((ValueError, KeyError), msg=key):
                summarize('\n'.join(['CREASE_COMMAND ' + json.dumps(changed)] + lines[1:]))

    def test_invalid_getter_geometry_selection_or_phase_cannot_qualify(self):
        report, lines = self.report_lines()
        row = report['getters']['preselected'][0]
        original = 'CREASE_GETTER ' + json.dumps(row)
        for key, bad in [('phase', 'unknown'), ('id', 'unknown'), ('maximum_error', True),
                         ('maximum_edge_sample_error', float('nan')),
                         ('maximum_edge_sample_error', 1.1e-6), ('history', False),
                         ('selection', False), ('identity', 1), ('attributes', False)]:
            changed = dict(row, **{key: bad})
            with self.assertRaises(ValueError, msg=key):
                summarize('\n'.join('CREASE_GETTER ' + json.dumps(changed) if s == original else s for s in lines))


if __name__ == '__main__':
    unittest.main()

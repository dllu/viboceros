"""Fixed native mixed-source evidence, panel interaction, staging and provenance."""
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
from .edge_analysis_mixed_cases import cases, membership_cases, panel_cases
from .edge_analysis_mixed_replay import FAMILIES
from .subd_edge_analysis_replay import HERE, read
from .structural_sdk import stage_helpers, validate_operations


class MixedEdgeAnalysisTests(unittest.TestCase):
    def test_frozen_requests_producers_and_shared_inputs(self):
        for family, recipes in zip(FAMILIES, (cases(), membership_cases(), panel_cases()), strict=True):
            path = HERE / 'fixtures' / (family + '.json')
            request = json.loads(path.read_bytes())
            validate_operations(request)
            self.assertEqual(request['operations'], recipes)
            data = read(family)
            self.assertEqual(data['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for name, digest in data['provenance']['sources'].items():
                raw = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(), digest, name)
            self.assertEqual([r['id'] for r in data['results']], [op['id'] for op in recipes])
        rows = {r['id']: r['value'] for r in read(FAMILIES[0])['results']}
        folder = HERE / 'fixtures/edge_analysis_mixed_inputs'
        manifest = json.loads((folder / 'provenance.json').read_bytes())
        self.assertEqual(len(manifest), 7)
        for name, item in manifest.items():
            raw = (folder / (name + '.3dm')).read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(), item['sha256'])
            self.assertEqual(raw, base64.b64decode(rows[item['case']]['source_archive']))

    def test_native_selection_groups_geometry_and_history(self):
        for op, row in zip(cases(), read(FAMILIES[0])['results'], strict=True):
            v = row['value']
            self.assertTrue(v['success'], op['id'])
            self.assertEqual([s['selected'] for s in v['before']['sources']], [op['preselect']] * len(v['before']['sources']))
            for phase in ('after', 'undo', 'redo'):
                for a, b in zip(v['before']['sources'], v[phase]['sources'], strict=True):
                    self.assertEqual(a['geometry'], b['geometry'])
                    self.assertEqual(b['name'], b['kind'])
                    self.assertEqual(b['groups'], int(op['grouped']))
                    selected = op['preselect'] and (op['command'] != 'ShowEdges' or b['kind'] == 'point')
                    self.assertEqual(b['selected'], selected, (op['id'], phase, b['kind']))
            if op['command'] == 'ShowEdges':
                self.assertEqual(v['after']['points'], [])
                self.assertEqual(v['redo']['points'], [[40., 50., 60.]])
                self.assertIn('Redoing Point', v['history'])
            else:
                self.assertEqual(len(v['after']['points']), 24 if op['actions'] == ['All', 'Mark'] else 6)
                self.assertEqual(v['undo']['points'], [])
                self.assertEqual(v['after']['points'], v['redo']['points'])
                self.assertIn('Undoing ZoomNaked', v['history'])
                self.assertIn('Redoing ZoomNaked', v['history'])

    def test_native_add_remove_preselection_and_enabled_panel_zoom(self):
        expected = [[False]*3, [False, True, True], [False, True, True], [True, False, False], [True, False, False], [False]*3]
        for row in read(FAMILIES[1])['results']:
            v = row['value']
            self.assertTrue(v['success'])
            self.assertEqual([s['action'] for s in v['stages']], ['Start', 'SelectRest', 'Add', 'SelectFirst', 'Remove', 'Zoom'])
            self.assertEqual([[o['selected'] for o in s['state']['sources']] for s in v['stages']], expected)
            trace = v['stages'][-1]['input']
            self.assertEqual(trace['errors'], [])
            self.assertEqual([s['token'] for s in trace['sent']], ['_All', '_Mark', ''])
            self.assertIn("Command: '_ZoomNaked", v['history'])
            self.assertIn('Found 12 edges total', v['history'])
            self.assertIn('Found 8 edges total', v['history'])
            self.assertEqual(len(v['after']['points']), 16)
            self.assertEqual(v['undo']['points'], [])
            self.assertEqual(v['redo']['points'], v['after']['points'])

    def test_native_panel_source_reversal_and_sequential_additions(self):
        for op, row in zip(panel_cases(), read(FAMILIES[2])['results'], strict=True):
            v = row['value']
            self.assertTrue(v['success'])
            self.assertEqual(v['stages'][-1]['input']['errors'], [])
            expected = 24 if op.get('panel_only') else 16
            self.assertEqual(len(v['after']['points']), expected)
            self.assertEqual(v['after']['points'], v['redo']['points'])
            self.assertEqual(v['undo']['points'], [])
            # In smooth mode the relative B-rep/SubD order reverses for panel Zoom.
            remaining = op['order'] if op.get('panel_only') else op['order'][1:]
            expected_order = sorted(reversed(remaining), key=lambda kind: kind == 'mesh')
            actual_order = ['mesh' if p[0] >= 20 else 'subd' if p[2] > 4.9 else 'brep'
                            for p in v['after']['points'][::8]]
            self.assertEqual(actual_order, expected_order, op['id'])

    def test_owned_validation_and_worker_staging(self):
        for op in cases() + membership_cases() + panel_cases():
            for change in (dict(order=['mesh']*3), dict(preselect=1), dict(flat=0), dict(membership=1), dict(extra=True), dict(actions=['Delete'])):
                with self.assertRaises(ValueError):
                    validate_operations(dict(operations=[dict(copy.deepcopy(op), **change)]))
        request = dict(protocol_version=1, iterations=1, operations=cases()+membership_cases()+panel_cases())
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(request, HERE, Path(folder))
            self.assertEqual((Path(folder) / 'fixtures/brep_direction/warped.3dm').read_bytes(), (HERE / 'fixtures/brep_direction/warped.3dm').read_bytes())
            subprocess.run([sys.executable, '-I', '-c', 'import sys;sys.path.insert(0,sys.argv[1]);import edge_analysis_mixed_probe', folder], check=True, capture_output=True)


if __name__ == '__main__':
    unittest.main()

"""Native geometry, negative controls, panel capabilities and frozen producers."""
import base64
import copy
import gzip
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from .subd_edge_analysis_cases import cases, controls, capabilities, panels, panels_mouse
from .subd_direction_analysis_cases import cases as admission_cases
from .subd_edge_analysis_replay import HERE, read
from .structural_sdk import stage_helpers, validate_operations

FAMILIES = [('subd_edge_analysis_discovery', cases()), ('subd_edge_analysis_controls', controls()),
            ('subd_edge_analysis_capabilities', capabilities()), ('subd_edge_analysis_panels', panels()),
            ('subd_edge_analysis_mouse', panels_mouse()), ('subd_direction_analysis_discovery', admission_cases())]


class SubDEdgeAnalysisTests(unittest.TestCase):
    def test_frozen_requests_producers_and_shared_inputs(self):
        for family, recipes in FAMILIES:
            path = HERE / 'fixtures' / (family + '.json')
            request = json.loads(path.read_bytes())
            validate_operations(request)
            self.assertEqual(request['operations'], recipes)
            data = read(family)
            self.assertEqual(data['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for name, digest in data['provenance']['sources'].items():
                content = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(content).hexdigest(), digest, name)
            self.assertEqual([r['id'] for r in data['results']], [op['id'] for op in recipes])
        folder = HERE / 'fixtures/subd_edge_analysis_inputs'
        for scene, item in json.loads((folder / 'provenance.json').read_bytes()).items():
            raw = (folder / (scene + '.3dm')).read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(), item['sha256'])
            data = read(item.get('family', 'subd_edge_analysis_controls'))
            row = next(r for r in data['results'] if r['id'] == item['case'])
            self.assertEqual(raw, base64.b64decode(row['value']['source_archive']))

    def test_native_selection_and_appearance_preserve_sources_and_pending_redo(self):
        for family, recipes in FAMILIES[:3]:
            for op, row in zip(recipes, read(family)['results'], strict=True):
                v = row['value']
                self.assertEqual(v['success'], op.get('finish') != 'Cancel')
                selected = op['command'] != 'ShowEdges' and op['preselect']
                for phase in ('after', 'undo', 'redo'):
                    self.assertEqual(v[phase]['selected'], selected, (op['id'], phase))
                    self.assertEqual(v[phase]['geometry'], v['before']['geometry'])
                if op['command'] == 'ShowEdges':
                    self.assertEqual(v['after']['points'], [])
                    self.assertEqual(v['redo']['points'], [[40., 50., 60.]])
                else:
                    self.assertEqual(v['undo']['points'], [])
                    self.assertEqual(v['after']['points'], v['redo']['points'])
                    self.assertIn('Undoing ZoomNaked', v['history'])
                    self.assertIn('Redoing ZoomNaked', v['history'])

    def test_native_all_mode_disables_zoom_and_diagnostics_retain_failed_inputs(self):
        for row in read('subd_edge_analysis_capabilities')['results']:
            for mode in row['value']['modes']:
                zoom = [r for r in mode['controls'] if r['label'].replace('&', '') == 'Zoom' and r['kind'] == 'Button']
                self.assertEqual(len(zoom), 1)
                self.assertEqual(zoom[0]['enabled'], mode['action'] != 'Show=All')
        for family in ('subd_edge_analysis_panels', 'subd_edge_analysis_mouse'):
            for row in read(family)['results']:
                self.assertIn('error', row['value']['modes'][-1]['input'])
                self.assertEqual(row['value']['after']['points'], [])
        for row in read('subd_direction_analysis_discovery')['results']:
            expected = row['id'].startswith('control_')
            self.assertEqual(row['value']['states'][1]['analysis'], expected)
            self.assertEqual(bool(row['value']['traces'][0]['controls']), expected)

    def test_owned_recipe_validation_and_worker_staging(self):
        for op in cases() + controls() + capabilities():
            for change in [dict(scene='../external'), dict(preselect=1), dict(flat=0), dict(shared_input=1), dict(extra=True), dict(actions=['Delete'])]:
                with self.assertRaises(ValueError):
                    validate_operations(dict(operations=[dict(copy.deepcopy(op), **change)]))
        request = dict(protocol_version=1, iterations=1, operations=cases())
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(request, HERE, Path(folder))
            subprocess.run([sys.executable, '-I', '-c', 'import sys;sys.path.insert(0,sys.argv[1]);import subd_edge_analysis_probe', folder], check=True, capture_output=True)


if __name__ == '__main__':
    unittest.main()

"""Immutable native provenance, continuity thresholds and owned split recipes."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .revolve_splitting_cases import cases, holdouts, mixed_cases
from .structural_sdk import stage_helpers, validate_operations
from .subd_edge_analysis_replay import HERE, read

FAMILIES = dict(revolve_splitting=cases, revolve_splitting_holdouts=holdouts,
                revolve_splitting_mixed=mixed_cases)


class RevolveSplittingTests(unittest.TestCase):
    def test_requests_and_frozen_native_producers(self):
        for family, make in FAMILIES.items():
            path = HERE / ('fixtures/' + family + '.json')
            query = json.loads(path.read_bytes())
            data = read(family)
            self.assertEqual(query['operations'], make())
            validate_operations(query)
            self.assertEqual([r['id'] for r in data['results']], [op['id'] for op in make()])
            self.assertEqual(data['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for name, digest in data['provenance']['sources'].items():
                raw = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(), digest, name)

    def test_public_commands_preserve_sources_selection_and_history(self):
        total = 0
        for family, make in FAMILIES.items():
            for op, row in zip(make(), read(family)['results'], strict=True):
                total += 1
                v = row['value']
                self.assertTrue(v['success'])
                self.assertNotIn('Unknown command:', v['history'])
                self.assertEqual([e['result'] for e in v['events']], ['Success'])
                self.assertEqual(v['created'], v['after'])
                self.assertEqual(v['before'], v['undo'])
                self.assertEqual(v['after'], v['redo'])
                self.assertEqual(len(v['after']), 1 if op['delete_input'] else 2)
                if not op['delete_input']:
                    self.assertEqual(v['before'][0], v['after'][0])
                out = v['after'][-1]
                self.assertFalse(out['source'])
                self.assertEqual(out['selected'], 0)
                self.assertEqual(out['brep']['faces'], len(out['brep']['surfaces']))
                self.assertEqual(out['brep']['faces'], len(v['full'][0]['faces']))
                if family != 'revolve_splitting':
                    self.assertEqual(v['document_tolerance'], dict(absolute=1e-6, relative=1e-12, angular=1e-6))
        self.assertEqual(total, 160)

    def test_continuity_is_geometric_and_split_options_have_distinct_thresholds(self):
        for op, row in zip(cases(), read('revolve_splitting')['results'], strict=True):
            name = op['id'].split('_')[0]
            split = op['split_at_tangents']
            crease = op['crease_splitting']
            faces = 1
            if name in ['corner', 'polyline'] and (split or crease):
                faces = 2
            if name == 'poles' and (split or crease):
                faces = 3
            if name == 'tangent' and split:
                faces = 2
            if name == 'rational' and split:
                faces = 3
            b = row['value']['after'][-1]['brep']
            self.assertEqual(b['faces'], faces, op['id'])
            self.assertEqual(b['solid'], name == 'poles', op['id'])
            if name == 'poles':
                self.assertEqual(b['trim_types'].count('Singular'), 2)
        for op, row in zip(holdouts(), read('revolve_splitting_holdouts')['results'], strict=True):
            if not op['id'].startswith('small_'):
                continue
            index = int(op['id'].split('_')[1])
            expected = 1 + int((op['crease_splitting'] and index >= 1) or (op['split_at_tangents'] and index == 3))
            self.assertEqual(row['value']['after'][-1]['brep']['faces'], expected, op['id'])

    def test_malformed_owned_recipes_are_rejected(self):
        for edit in [dict(id='unowned'), dict(deformable=1), dict(crease_splitting=0),
                     dict(split_at_tangents='Yes'), dict(point_count=True), dict(extra=True)]:
            with self.assertRaises(ValueError):
                validate_operations(dict(operations=[dict(cases()[0], **edit)]))
        op = copy.deepcopy(cases()[0])
        op['curve']['control_points'][0]['weight'] = True
        with self.assertRaises(ValueError):
            validate_operations(dict(operations=[op]))

    def test_helpers_stage_and_import_without_native_runtime(self):
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(dict(operations=[cases()[0]]), HERE, Path(directory))
            subprocess.run([sys.executable, '-I', '-c',
                            'import sys;sys.path.insert(0,sys.argv[1]);import revolve_splitting_probe',
                            directory], check=True, capture_output=True)


if __name__ == '__main__':
    unittest.main()

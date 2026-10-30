"""Authored inputs, retained native data, launch guards and current receipts."""
import base64
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from .capture_evidence import HERE, frozen, measurement_digest
from .client import OracleClient, OracleProtocolError
from .generate_subd_cap_regions import request as regions
from .generate_subd_cap_holdouts import request as holdouts
from .structural_sdk import stage_helpers, validate_operations
from .subd_cap_regions_cases import CASES, validate

FAMILIES = {'subd_cap_regions': (regions, 18), 'subd_cap_holdouts': (holdouts, 8)}


def captures():
    return {name: json.loads((HERE/'observations'/(name+'.json')).read_text()) for name in FAMILIES}


class SubDCapTests(unittest.TestCase):
    def test_authored_requests_frozen_producers_and_source_archives(self):
        for family, record in captures().items():
            builder, count = FAMILIES[family]
            request = builder()
            validate_operations(request)
            raw = (HERE/'fixtures'/(family+'.json')).read_bytes()
            self.assertEqual(json.loads(raw), request)
            self.assertEqual(hashlib.sha256(raw).hexdigest(), record['provenance']['request_sha256'])
            self.assertEqual(len(record['results']), count)
            self.assertEqual([o['id'] for o in request['operations']], [r['id'] for r in record['results']])
            for name in record['provenance']['sources']:
                frozen(record, name)
            namespace = {}
            exec(compile(frozen(record, 'subd_cap_regions_cases.py'), 'frozen authored SubD cases', 'exec'), namespace)
            for row in record['results']:
                self.assertEqual(namespace['CASES'][row['id']], CASES[row['id']])
                self.assertEqual(row['value']['input'], CASES[row['id']])
                data = base64.b64decode(row['value']['source_3dm'], validate=True)
                self.assertTrue(data.startswith(b'3D Geometry File Format'))
                self.assertEqual((HERE/'fixtures'/family/(row['id']+'.3dm')).read_bytes(), data)

    def test_all_native_stages_keep_valid_archives_and_original_id_prefixes(self):
        for record in captures().values():
            for row in record['results']:
                value = row['value']
                self.assertTrue(value['success'])
                for i, source in enumerate(value['input']['sources']):
                    before = next(o['subd'] for o in value['before'] if o['source'] == i)
                    self.assertEqual(before['points'], source['vertices'])
                    self.assertEqual(before['faces'], source['faces'])
                    for phase in ['before', 'after', 'after_script', 'undo', 'redo']:
                        for obj in value[phase]:
                            if 'subd' not in obj:
                                continue
                            subd = obj['subd']
                            self.assertTrue(subd['valid'])
                            self.assertTrue(base64.b64decode(subd['geometry_3dm'], validate=True).startswith(b'3D Geometry File Format'))
                            for field in ['vertex_ids', 'edge_ids', 'face_ids']:
                                self.assertEqual(len(set(subd[field])), len(subd[field]))
                                self.assertEqual(subd[field][:len(before[field])], before[field])

    def test_world_bounding_box_fan_centers_and_degenerate_native_opening_are_retained(self):
        records = {r['id']: r['value'] for c in captures().values() for r in c['results']}
        for name in ['pentagon-triangles', 'skew-quad', 'skew-pentagon', 'tilted-skew', 'diagonal-quad']:
            value = records[name]
            before = next(o['subd'] for o in value['before'] if o['source'] == 0)
            after = next(o['subd'] for o in value['after'] if o['source'] == 0)
            points = before['points']; n = len(points)//2
            centers = [tuple((min(p[k] for p in ring)+max(p[k] for p in ring))/2 for k in range(3)) for ring in [points[:n], points[n:]]]
            self.assertEqual(set(map(tuple, after['points'][2*n:])), set(centers))
        diagonal = next(o['subd'] for o in records['diagonal-quad']['after'] if o['source'] == 0)
        self.assertEqual(diagonal['edge_face_counts'].count(1), 6)
        self.assertEqual(len(diagonal['faces']), 10)
        self.assertTrue(diagonal['valid'])

    def test_copy_and_noop_history_are_retained(self):
        rows = captures()['subd_cap_regions']['results']
        for row in rows:
            value = row['value']
            if row['id'] == 'copy':
                output = next(o for o in value['after'] if 'subd' in o and o['source'] is None)
                self.assertEqual(output['groups'], [])
                self.assertFalse(output['selected'])
                before = next(o for o in value['before'] if o['source'] == 0)
                after = next(o for o in value['after'] if o['source'] == 0)
                self.assertEqual(before['groups'], after['groups'])
            if row['id'] in ['sheet', 'closed-noop']:
                for field in ['points', 'faces', 'vertex_tags', 'edge_tags', 'vertex_ids', 'edge_ids', 'face_ids']:
                    before = next(o['subd'] for o in value['before'] if o['source'] == 0)
                    after = next(o['subd'] for o in value['after'] if o['source'] == 0)
                    self.assertEqual(before[field], after[field])
                self.assertFalse(any(o['name'] == 'Undo witness' for o in value['undo']))

    def test_invalid_or_shared_launches_are_rejected_before_host_start(self):
        query = regions()
        for operation in [{}, dict(op='subd_cap_regions', id='unknown', case='unknown'), dict(query['operations'][0], vertices=[])]:
            with self.assertRaises(ValueError):
                validate(operation)
        for scheme, request in [(None, query), ('VibocerosOracleSubDCapGuard', dict(query, iterations=True)),
                ('VibocerosOracleSubDCapGuard', dict(query, iterations=2)),
                ('VibocerosOracleSubDCapGuard', dict(query, operations=query['operations']*65)),
                ('VibocerosOracleSubDCapGuard', dict(query, operations=query['operations']+[dict(op='curve_length', id='other')]))]:
            with mock.patch.dict(os.environ, {'DISPLAY': ':101', 'VIBOCEROS_ORACLE_HEADLESS': ':101'}), mock.patch('tools.rhino_oracle.client._rhino_process_ids', return_value=set()), mock.patch('tools.rhino_oracle.client._run_logged') as launch, self.assertRaisesRegex(OracleProtocolError, 'private dedicated'):
                OracleClient(launcher='/bin/true', settings_scheme=scheme).run_rhino(copy.deepcopy(request))
            launch.assert_not_called()

    def test_staging_contains_all_shared_scene_dependencies(self):
        for builder, _ in FAMILIES.values():
            with tempfile.TemporaryDirectory() as path:
                stage_helpers(builder(), HERE, Path(path))
                for name in ['subd_cap_regions_probe', 'subd_cap_regions_cases', 'crease_probe', 'crease_cases', 'subd_box_probe', 'subdivide_probe', 'join_probe', 'merge_edges_probe']:
                    self.assertTrue((Path(path)/(name+'.py')).is_file(), name)

    def test_current_receipt_and_historical_measurements_remain_bound(self):
        from .qualification_sources import SUBD_CAP_MODULES, with_shared_sources
        root = HERE.parent.parent
        receipt = json.loads((root/'docs/geometry/subd-cap-native-validation.json').read_text())
        self.assertEqual(receipt['native_command_workflows'], 26)
        self.assertEqual(receipt['coordinate_epsilon'], 1e-6)
        for path in SUBD_CAP_MODULES:
            self.assertTrue(set(SUBD_CAP_MODULES) <= set(with_shared_sources([path])))
            self.assertIn(path, receipt['source_hashes'])
        for path, digest in receipt['source_hashes'].items():
            self.assertEqual(hashlib.sha256((root/path).read_bytes()).hexdigest(), digest, path)
        old = json.loads((root/'docs/geometry/mesh-closest-precision-validation.json').read_text())
        self.assertEqual(old['qualified_revision'], receipt['base_revision'])
        self.assertEqual(measurement_digest(old), '253e29ad8f9f45b88a5239dae2d6e7b06f1a626d356bdac51faa42d465ca3286')
        self.assertEqual(measurement_digest(old), receipt['historical_closest_measurements_sha256'])
        for family, record in captures().items():
            for directory in ['fixtures', 'observations']:
                self.assertEqual(hashlib.sha256((HERE/directory/(family+'.json')).read_bytes()).hexdigest(), receipt['families'][family][directory])
        self.assertEqual(receipt['retained_gaps'], json.loads((HERE/'fixtures/subd_cap_retained_gaps.json').read_text()))

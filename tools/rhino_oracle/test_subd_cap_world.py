"""Independent planar-metric inputs, native circle evidence and source binding."""
import base64
import copy
from fractions import Fraction
import hashlib
from itertools import permutations
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from .capture_evidence import HERE, frozen, measurement_digest
from .client import OracleClient, OracleProtocolError
from .generate_subd_cap_annuli import request as annuli
from .generate_subd_cap_metric import request as metric
from .subd_cap_annuli_cases import CASES as ANNULI_CASES, validate as validate_annuli
from .subd_cap_metric_cases import CASES as METRIC_CASES, validate as validate_metric
from .structural_sdk import stage_helpers, validate_operations

FAMILIES = {'subd_cap_annuli': (annuli, ANNULI_CASES, 20), 'subd_cap_metric': (metric, METRIC_CASES, 12)}


def captures():
    return {name: json.loads((HERE/'observations'/(name+'.json')).read_text()) for name in FAMILIES}


def circle_sign(points):
    # Independent full lifted determinant, retaining all three squared world
    # coordinates. XY is a nonsingular chart in these authored holdouts.
    points = [[Fraction(x) for x in p] for p in points]
    matrix = [[p[0], p[1], sum(x*x for x in p), Fraction(1)] for p in points]
    result = Fraction(0)
    for order in permutations(range(4)):
        inversions = sum(order[i] > order[j] for i in range(4) for j in range(i+1, 4))
        term = Fraction(1)
        for row, column in enumerate(order):
            term *= matrix[row][column]
        result += term if inversions % 2 == 0 else -term
    a, b, c = points[:3]
    orientation = (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])
    assert orientation != 0
    return 0 if result == 0 else 1 if result*orientation > 0 else -1


def native_circle_signs(value):
    before = next(o['subd'] for o in value['before'] if o['source'] == 0)
    after = next(o['subd'] for o in value['after'] if o['source'] == 0)
    uses = {}
    for face in after['faces'][len(before['faces']):]:
        assert len(face) == 3
        for i, a in enumerate(face):
            uses.setdefault(tuple(sorted([a, face[(i+1)%3]])), []).append(face[(i+2)%3])
    return [circle_sign([before['points'][i] for i in [a, b, *opposite]])
            for (a, b), opposite in uses.items() if len(opposite) == 2]


class SubDCapWorldTests(unittest.TestCase):
    def test_authored_requests_frozen_producers_and_exact_input_archives(self):
        for family, record in captures().items():
            builder, cases, count = FAMILIES[family]
            query = builder(); validate_operations(query)
            raw = (HERE/'fixtures'/(family+'.json')).read_bytes()
            self.assertEqual(json.loads(raw), query)
            self.assertEqual(hashlib.sha256(raw).hexdigest(), record['provenance']['request_sha256'])
            self.assertEqual(len(record['results']), count)
            self.assertEqual([o['id'] for o in query['operations']], [r['id'] for r in record['results']])
            for name in record['provenance']['sources']:
                frozen(record, name)
            for row in record['results']:
                value = row['value']; self.assertTrue(value['success'])
                self.assertEqual(value['input'], cases[row['id']])
                data = base64.b64decode(value['source_3dm'], validate=True)
                self.assertTrue(data.startswith(b'3D Geometry File Format'))
                self.assertEqual((HERE/'fixtures'/family/(row['id']+'.3dm')).read_bytes(), data)
                source = value['input']['sources'][0]
                before = next(o['subd'] for o in value['before'] if o['source'] == 0)
                self.assertEqual(before['points'], source['vertices'])
                self.assertEqual(before['faces'], source['faces'])

    def test_every_native_lifecycle_stage_retains_valid_archives_and_original_ids(self):
        for record in captures().values():
            for row in record['results']:
                value = row['value']; before = next(o['subd'] for o in value['before'] if o['source'] == 0)
                for phase in ['before', 'after', 'after_script', 'undo', 'redo']:
                    subd = next(o['subd'] for o in value[phase] if o['source'] == 0)
                    self.assertTrue(subd['valid'])
                    self.assertTrue(base64.b64decode(subd['geometry_3dm'], validate=True).startswith(b'3D Geometry File Format'))
                    for field in ['vertex_ids', 'edge_ids', 'face_ids']:
                        self.assertEqual(subd[field][:len(before[field])], before[field])
                after = next(o['subd'] for o in value['after'] if o['source'] == 0)
                self.assertNotIn(1, after['edge_face_counts'])

    def test_metric_holdouts_have_strict_world_distance_delaunay_certificates(self):
        record = captures()['subd_cap_metric']
        for row in record['results']:
            signs = native_circle_signs(row['value'])
            self.assertGreater(len(signs), 0)
            self.assertTrue(all(sign == -1 for sign in signs), row['id'])

    def test_symmetric_native_holdouts_retain_exact_circle_ties(self):
        rows = captures()['subd_cap_annuli']['results']
        centered = next(r for r in rows if r['id'] == 'centered-crease')
        self.assertEqual(native_circle_signs(centered['value']).count(0), 8)
        for row in rows:
            self.assertNotIn(1, native_circle_signs(row['value']), row['id'])

    def test_invalid_and_shared_requests_cannot_launch_the_host(self):
        for builder, validate in [(annuli, validate_annuli), (metric, validate_metric)]:
            query = builder()
            for operation in [{}, dict(query['operations'][0], vertices=[]), dict(query['operations'][0], id='unknown')]:
                with self.assertRaises(ValueError): validate(operation)
            for scheme, request in [(None, query), ('VibocerosOracleWorldCapGuard', dict(query, iterations=True)),
                    ('VibocerosOracleWorldCapGuard', dict(query, iterations=2)),
                    ('VibocerosOracleWorldCapGuard', dict(query, operations=query['operations']*65)),
                    ('VibocerosOracleWorldCapGuard', dict(query, operations=query['operations']+[dict(op='curve_length', id='other')]))]:
                with mock.patch.dict(os.environ, {'DISPLAY': ':101', 'VIBOCEROS_ORACLE_HEADLESS': ':101'}), mock.patch('tools.rhino_oracle.client._rhino_process_ids', return_value=set()), mock.patch('tools.rhino_oracle.client._run_logged') as launch, self.assertRaisesRegex(OracleProtocolError, 'private dedicated'):
                    OracleClient(launcher='/bin/true', settings_scheme=scheme).run_rhino(copy.deepcopy(request))
                launch.assert_not_called()

    def test_staging_contains_all_transitive_probe_modules(self):
        for family, (builder, _, _) in FAMILIES.items():
            with tempfile.TemporaryDirectory() as path:
                stage_helpers(builder(), HERE, Path(path))
                for name in [family+'_probe', family+'_cases', 'subd_cap_regions_probe', 'subd_cap_regions_cases', 'crease_probe', 'crease_cases', 'subd_box_probe', 'subdivide_probe', 'join_probe', 'merge_edges_probe']:
                    self.assertTrue((Path(path)/(name+'.py')).is_file(), name)

    def test_current_sources_and_historical_measurements_remain_bound(self):
        from .qualification_sources import SUBD_CAP_MODULES, with_shared_sources
        root = HERE.parent.parent
        report = json.loads((root/'docs/geometry/subd-cap-world-metric-validation.json').read_text())
        self.assertEqual(report['native_command_workflows'], 58)
        self.assertEqual(report['coordinate_epsilon'], 1e-6)
        for name in SUBD_CAP_MODULES:
            self.assertTrue(set(SUBD_CAP_MODULES) <= set(with_shared_sources([name])))
            self.assertIn(name, report['source_hashes'])
        for name, digest in report['source_hashes'].items():
            self.assertEqual(hashlib.sha256((root/name).read_bytes()).hexdigest(), digest, name)
        historical = json.loads((root/'docs/geometry/subd-cap-native-validation.json').read_text())
        self.assertEqual(historical['qualified_revision'], report['base_revision'])
        # This record already had source_hashes_scope before freezing. Preserve
        # that original field too; only these two annotations were added.
        digest = measurement_digest(historical, {'qualified_revision', 'current_source_receipt'})
        self.assertEqual(digest, '09b2ac64d964981a056368364dcfcd1cc9c025bff6aea3846c63406c92b50289')
        self.assertEqual(digest, report['historical_subd_measurements_sha256'])
        self.assertEqual(report['additional_retained_gaps'], json.loads((HERE/'fixtures/subd_cap_world_retained_gaps.json').read_text()))
        for family in FAMILIES:
            for directory in ['fixtures', 'observations']:
                self.assertEqual(hashlib.sha256((HERE/directory/(family+'.json')).read_bytes()).hexdigest(), report['families'][family][directory])

"""Outline producer isolation, projection evidence and measured boundary checks."""
from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .mesh_outline_cases import FAMILIES, request, validate
from .report_mesh_outline import ROOT, HERE, PROJECTION_GAPS, PRECISION_GAPS, native, summarize
from .structural_sdk import stage_helpers, validate_operations


class MeshOutlineTests(unittest.TestCase):
    def test_fixed_requests_staged_helpers_and_immutable_captures(self):
        _, rows, qualified, _ = native()
        self.assertEqual((len(rows), len(qualified)), (231, 196))
        for family in FAMILIES:
            payload = request(family)
            validate_operations(payload)
            with tempfile.TemporaryDirectory() as directory:
                stage_helpers(payload, HERE, Path(directory))
                subprocess.run([sys.executable, '-I', '-c',
                                'import sys;sys.path.insert(0,sys.argv[1]);import mesh_outline_probe', directory],
                               check=True, capture_output=True)
            paths = [HERE / directory / ('mesh_outline_' + family + '.json') for directory in ['fixtures', 'observations']]
            before = [p.read_bytes() for p in paths]
            result = subprocess.run([sys.executable, '-m', 'tools.rhino_oracle.generate_mesh_outline']
                                    + (['--' + family] if family != 'sdk' else []), cwd=ROOT, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('observations are immutable', result.stderr)
            self.assertEqual([p.read_bytes() for p in paths], before)
        for invalid in [None, [], {}, dict(request()['operations'][0], case='arbitrary'),
                        dict(request()['operations'][0], path='arbitrary')]:
            with self.assertRaises(ValueError):
                validate(invalid)

    def test_native_holes_overlap_and_null_empty_images(self):
        _, rows, _, _ = native()
        self.assertEqual(len(rows['outline_plane_frame']['outlines']), 2)
        self.assertEqual(len(rows['outline_plane_point_touch']['outlines']), 2)
        self.assertEqual(len(rows['outline_plane_overlap']['outlines']), 1)
        self.assertEqual(len(rows['outline_plane_overlap']['outlines'][0]), 9)
        self.assertIs(rows['outline_plane_edge_on']['returned_null'], True)
        self.assertEqual(rows['outline_plane_edge_touch']['outlines'][0],
                         [[0.,0.,0.], [4.,0.,0.], [8.,0.,0.], [8.,3.,0.], [4.,3.,0.], [0.,3.,0.], [0.,0.,0.]])

    def test_recorded_viewports_use_clip_center_or_aligned_cplane(self):
        _, rows, required, _ = native()
        for name, value in rows.items():
            if value['input']['method'] == 'plane' or name not in required:
                continue
            view = value['view']
            length = math.sqrt(sum(x*x for x in view['direction']))
            direction = [x/length for x in view['direction']]
            normal = value['plane']['z']
            if not view['perspective'] and abs(abs(sum(a*b for a,b in zip(direction,normal))) - 1.) < 1e-12:
                distance = lambda p: sum((a-b)*c for a,b,c in zip(p,value['plane']['origin'],normal))
            else:
                near, far = view['frustum'][4:]
                depth = 2./(1./near + 1./far) if view['perspective'] else (near+far)*.5
                distance = lambda p: sum((a-b)*c for a,b,c in zip(p,view['location'],direction)) - depth
            for polyline in value['outlines']:
                for point in polyline:
                    self.assertLess(abs(distance(point)), 1e-10)

    def test_quad_outline_depends_on_far_clipping_not_tolerance_or_face_order(self):
        _, rows, _, _ = native()
        for name, value in rows.items():
            if name.startswith('outline_scale_'):
                expected = 5 if name.endswith('_ac') else 4
                self.assertEqual([len(p) for p in value['outlines']], [expected], name)
            if name.startswith('outline_context_') and name.endswith('_bd'):
                self.assertEqual([len(p) for p in value['outlines']], [4], name)
        low = rows['outline_facing_far_43']
        high = rows['outline_facing_far_44']
        changed = deepcopy(low['input'])
        changed['frustum'][-1] = 44.
        self.assertEqual(changed, high['input'])
        self.assertEqual([len(p) for p in low['outlines']], [5])
        self.assertEqual([len(p) for p in high['outlines']], [4])
        for name, value in rows.items():
            if name.startswith('outline_facing_order_'):
                expected = 5 if name.startswith('outline_facing_order_40_') else 4
                self.assertEqual([len(p) for p in value['outlines']], [expected], name)

    def evidence(self):
        record = json.loads((ROOT / 'docs/mesh-outline-qualification.json').read_text())
        return record, ['MESH_OUTLINE_SDK ' + json.dumps(row) for row in record['geometry']]

    def test_report_recomputes_measured_contours_and_source_bindings(self):
        record, lines = self.evidence()
        self.assertEqual(reconstruct_measurement(record, summarize, '\n'.join(reversed(lines))), record)
        self.assertEqual(record['replayed_geometry_workflows'], 196)
        self.assertEqual(record['qualified_geometry_workflows'], 176)
        self.assertEqual(record['projection_gaps'], sorted(PROJECTION_GAPS))
        self.assertEqual(record['projection_precision_gaps'], sorted(PRECISION_GAPS))
        self.assertEqual(len(record['exploratory_viewport_workflows_without_recorded_frustum']), 35)
        self.assertTrue(record['pending'])
        self.assertLess(record['max_absolute_error'], 1e-7)
        with self.assertRaises(ValueError):
            summarize('\n'.join(lines[:-1]))
        with self.assertRaises(ValueError):
            summarize('\n'.join(lines + lines[:1]))
        for field, value in [('source_unchanged', False), ('max_error', float('nan')),
                             ('outlines', []), ('max_error', .01)]:
            rows = deepcopy(record['geometry'])
            row = next(r for r in rows if r['outlines'] and r['max_error'] is not None)
            row[field] = value
            with self.assertRaises(ValueError):
                summarize('\n'.join('MESH_OUTLINE_SDK ' + json.dumps(r) for r in rows))
        rows = deepcopy(record['geometry'])
        row = next(r for r in rows if r['outlines'])
        row['outlines'][0][1][0] += .1
        with self.assertRaises(ValueError):
            summarize('\n'.join('MESH_OUTLINE_SDK ' + json.dumps(r) for r in rows))
        for field, value in [('max_error', 0.), ('outlines', [])]:
            rows = deepcopy(record['geometry'])
            row = next(r for r in rows if r['case'] in PROJECTION_GAPS)
            row[field] = value
            with self.assertRaises(ValueError):
                summarize('\n'.join('MESH_OUTLINE_SDK ' + json.dumps(r) for r in rows))


if __name__ == '__main__':
    unittest.main()

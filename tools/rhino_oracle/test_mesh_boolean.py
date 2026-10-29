"""Frozen producer isolation and independently measured polygon Boolean checks."""
from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .mesh_boolean_cases import request, validate
from .report_mesh_boolean import HERE, ROOT, native, summarize
from .structural_sdk import stage_helpers, validate_operations


class MeshBooleanTests(unittest.TestCase):
    def test_fixed_requests_standalone_helpers_and_immutable_observations(self):
        _, rows, _ = native()
        self.assertEqual(len(rows), 48)
        for controls in [False, True]:
            query = request(controls=controls)
            validate_operations(query)
            with tempfile.TemporaryDirectory() as folder:
                stage_helpers(query, HERE, Path(folder))
                subprocess.run([sys.executable, '-I', '-c',
                                'import sys;sys.path.insert(0,sys.argv[1]);import mesh_boolean_probe', folder],
                               check=True, capture_output=True)
            family = 'mesh_boolean_controls' if controls else 'mesh_boolean_sdk'
            paths = [HERE/d/(family+'.json') for d in ['fixtures', 'observations']]
            before = [p.read_bytes() for p in paths]
            result = subprocess.run([sys.executable, '-m', 'tools.rhino_oracle.generate_mesh_boolean']
                                    + (['--controls'] if controls else []), cwd=ROOT, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('observations are immutable', result.stderr)
            self.assertEqual(before, [p.read_bytes() for p in paths])
        for invalid in [None, [], {}, dict(request()['operations'][0], case=[]),
                        dict(request()['operations'][0], path='arbitrary')]:
            with self.assertRaises(ValueError):
                validate(invalid)

    def test_native_coincident_repeats_verify_actual_unchanged_inputs(self):
        _, rows, _ = native()
        for op in ['union', 'intersection', 'difference']:
            values = [rows['coincident_repeat_{}_{}'.format(i, op)] for i in range(3)]
            self.assertTrue(all(v['meshes'] == values[0]['meshes'] for v in values))
            self.assertTrue(all(v['source_meshes_before'] == v['source_meshes_after'] == v['input']['meshes'] for v in values))
        self.assertEqual(rows['overlap_union']['volumes'], [42.])
        self.assertEqual(rows['overlap_intersection']['volumes'], [6.])
        self.assertEqual(rows['overlap_difference']['volumes'], [18.])

    def test_report_recomputes_geometry_and_rejects_stale_or_forged_measurements(self):
        record = json.loads((ROOT/'docs/mesh-boolean-qualification.json').read_text())
        lines = ['MESH_BOOLEAN_SDK '+json.dumps(r) for r in record['geometry']]
        self.assertEqual(reconstruct_measurement(record, summarize, '\n'.join(reversed(lines))), record)
        self.assertEqual((record['replayed'], record['matched']), (48, 26))
        for bad in [lines[:-1], lines+lines[:1]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(bad))
        for field, value in [('source_unchanged', False), ('area', 123.),
                             ('signed_volume', float('nan')), ('maximum_sampled_boundary_deviation', .01),
                             ('meshes', []), ('kernel_error', 'failure'), ('geometry_matches', False)]:
            rows = deepcopy(record['geometry'])
            row = next(r for r in rows if r['case'] == 'overlap_union')
            row[field] = value
            with self.assertRaises(ValueError):
                summarize('\n'.join('MESH_BOOLEAN_SDK '+json.dumps(r) for r in rows))


if __name__ == '__main__':
    unittest.main()

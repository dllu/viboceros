"""Frozen command captures, getter lifecycle and independent output audits."""
from copy import deepcopy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .mesh_boolean_command_cases import FAMILIES, request, validate
from .report_mesh_boolean_commands import HERE, ROOT, native, summarize
from .structural_sdk import stage_helpers, validate_operations


class MeshBooleanCommandTests(unittest.TestCase):
    def test_fixed_requests_standalone_helpers_saved_sources_and_immutable_captures(self):
        _, rows, exploratory, _ = native()
        self.assertEqual((len(rows), len(exploratory)), (85, 49))
        for family in FAMILIES:
            payload = request(family)
            validate_operations(payload)
            with tempfile.TemporaryDirectory() as folder:
                stage_helpers(payload, HERE, Path(folder))
                subprocess.run([sys.executable, '-I', '-c',
                                'import sys;sys.path.insert(0,sys.argv[1]);import mesh_boolean_command_probe', folder],
                               check=True, capture_output=True)
            paths = [HERE/d/('mesh_boolean_'+family+'.json') for d in ['fixtures', 'observations']]
            before = [p.read_bytes() for p in paths]
            result = subprocess.run([sys.executable, '-m', 'tools.rhino_oracle.generate_mesh_boolean', '--'+family],
                                    cwd=ROOT, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('observations are immutable', result.stderr)
            self.assertEqual(before, [p.read_bytes() for p in paths])
        for bad in [None, [], {}, dict(request()['operations'][0], case=[]),
                    dict(request()['operations'][0], path='arbitrary')]:
            with self.assertRaises(ValueError):
                validate(bad)

    def test_public_native_tolerance_policy_and_cross_command_preference_capture(self):
        _, rows, _, _ = native()
        policy = rows['boolean_policy_auto_defaults']['tolerance_policy']
        self.assertEqual(policy, dict(zero=2.**-32, coefficient=1e-4))
        for name in ['intersection_after_document', 'difference_after_document', 'difference_after_cancel']:
            self.assertIn('Tolerance=Document', rows['boolean_tolerance_'+name]['history'])
        for name in ['intersection_after_zero', 'difference_after_zero']:
            self.assertIn('Tolerance=Zero', rows['boolean_tolerance_'+name]['history'])
        self.assertIn('Tolerance=Auto', rows['boolean_tolerance_union_after_auto']['history'])

    def test_report_recomputes_metadata_geometry_and_rejects_forged_measurements(self):
        record = json.loads((ROOT/'docs/mesh-boolean-command-qualification.json').read_text())
        log = lambda key, prefix: '\n'.join(prefix+' '+json.dumps(r) for r in record[key])
        commands, getters = log('commands', 'MESH_BOOLEAN_COMMAND'), log('getters', 'MESH_BOOLEAN_GETTER')
        self.assertEqual(summarize(commands, getters), record)
        for bad in [commands.splitlines()[:-1], commands.splitlines()+commands.splitlines()[:1]]:
            with self.assertRaises(ValueError):
                summarize('\n'.join(bad), getters)
        for field, value in [('sources_restored_on_undo', False), ('success', False)]:
            values = deepcopy(record['commands'])
            next(r for r in values if r['id'] == 'boolean_lifecycle_union_overlap_pre')[field] = value
            with self.assertRaises(ValueError):
                summarize('\n'.join('MESH_BOOLEAN_COMMAND '+json.dumps(r) for r in values), getters)
        for mutation in ['metadata', 'distance', 'mesh', 'point', 'label']:
            values = deepcopy(record['getters'])
            stage = next(r for r in values if r['id'] == 'boolean_lifecycle_union_overlap_pre')['after']
            if mutation == 'metadata':
                stage['objects'][-1]['layer'] = 'forged'
            elif mutation == 'distance':
                stage['maximum_sampled_boundary_deviation'] = float('nan')
            elif mutation == 'mesh':
                stage['objects'][-1]['mesh']['faces'].pop()
            elif mutation == 'point':
                stage['objects'][0]['point'][0] += 1
            else:
                stage['geometry'] = False
            with self.assertRaises(ValueError):
                summarize(commands, '\n'.join('MESH_BOOLEAN_GETTER '+json.dumps(r) for r in values))


if __name__ == '__main__':
    unittest.main()

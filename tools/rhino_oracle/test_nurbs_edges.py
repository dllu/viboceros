"""Immutable native edge commands, SDK curve equivalence and strict owned inputs."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .nurbs_edge_command_probe import ASSETS, cases, discovery_cases, interaction_cases
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).resolve().parent


class NurbsEdgesTests(unittest.TestCase):
    def test_native_recipes_hashes_sdk_curves_and_lifecycle(self):
        workflows = curves = 0
        for family, recipes in [('nurbs_edge_command_discovery', discovery_cases()),
                                ('nurbs_edge_command', cases()),
                                ('nurbs_edge_nested', interaction_cases())]:
            path = HERE/'fixtures'/(family+'.json')
            query = json.loads(path.read_bytes())
            data = json.loads(gzip.decompress((HERE/'observations'/(family+'.json.gz')).read_bytes()))
            self.assertEqual(query['operations'], recipes)
            validate_operations(query)
            self.assertEqual([r['id'] for r in data['results']], [r['id'] for r in recipes])
            self.assertEqual(data['provenance']['request_sha256'], hashlib.sha256(path.read_bytes()).hexdigest())
            for name, digest in data['provenance']['sources'].items():
                raw = gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(raw).hexdigest(), digest, name)
            for op, row in zip(recipes, data['results']):
                workflows += 1
                value = row['value']
                cancel = op.get('cancel', False)
                self.assertEqual(value['success'], not cancel)
                self.assertEqual(value['results'], ['Cancel' if cancel else 'Success'])
                source = value['before'][0]
                for phase in ('settled', 'undo', 'redo'):
                    owner = next(o for o in value[phase] if o['source'] == 0)
                    self.assertEqual(owner['crc'], source['crc'])
                    self.assertEqual(owner['groups'], source['groups'])
                    self.assertTrue(all(not o['components'] for o in value[phase]))
                if cancel:
                    self.assertEqual(len(value['settled']), len(value['before']))
                    continue
                outputs = [o for o in value['settled'] if o['source'] is None][:len(value['selected'])]
                self.assertEqual(len(outputs), len(value['sdk']))
                for output, sdk in zip(outputs, value['sdk']):
                    curves += 1
                    self.assertEqual(output['kind'], 'NurbsCurve')
                    self.assertFalse(output['selected'])
                    for name in ('name', 'layer', 'groups', 'color', 'color_source'):
                        self.assertEqual(output[name], source[name])
                    if not op.get('flat'):
                        self.assertEqual(output['curve'], sdk['curves'][-1])
                    else:
                        self.assertEqual(output['curve']['degree'], 1)
                        self.assertEqual(len(output['curve']['control_points']), 2)
                for phase in ('settled', 'redo'):
                    self.assertEqual([(o['source'], o['kind'], o['selected']) for o in value[phase]],
                                     [(o['source'], o['kind'], o['selected']) for o in value['settled']])
                self.assertNotIn('SubDOptions', value['history'])
                self.assertNotIn('MeshOptions', value['history'])
        self.assertEqual((workflows, curves), (26, 307))

    def test_strict_recipes_and_isolated_worker_assets(self):
        query = json.loads((HERE/'fixtures/nurbs_edge_command.json').read_bytes())
        for change in [dict(kind='external'), dict(delete=0), dict(all=1), dict(extra=True),
                       dict(flat=1), dict(whole_first=0), dict(nested=1)]:
            op = copy.deepcopy(query['operations'][0])
            op.update(change)
            with self.assertRaises(ValueError):
                validate_operations(dict(query, operations=[op]))
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(query, HERE, Path(folder))
            for asset in ASSETS:
                relative = Path('fixtures/subd_soft_nurbs_inputs')/(asset+'.3dm')
                self.assertEqual((Path(folder)/relative).read_bytes(), (HERE/relative).read_bytes())
            subprocess.run([sys.executable, '-I', '-c',
                            'import sys;sys.path.insert(0,sys.argv[1]);import nurbs_edge_command_probe', folder],
                           check=True, capture_output=True)


if __name__ == '__main__':
    unittest.main()

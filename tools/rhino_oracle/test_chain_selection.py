"""Owned native chain recipes, immutable provenance and worker staging."""
import copy
import gzip
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from .chain_selection_cases import (
    discovery_cases, curvature_cases, curvature_limit_cases, curvature_boundary_cases,
    acceptance_cases, fork_cases, angle_precision_cases, option_minimum_cases,
    option_boundary_cases, source,
)
from .generate_chain_selection_controls import grouped
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).resolve().parent
FAMILIES = [('discovery', discovery_cases())] + [
    (g, grouped(g)) for g in (
        'lines', 'reversed', 'corner', 'cycle', 'surface', 'mixed',
        'tolerance', 'lifecycle', 'branch', 'prefer_curve',
    )
] + [
    ('curvature', curvature_cases()), ('curvature_limits', curvature_limit_cases()),
    ('curvature_boundary', curvature_boundary_cases()), ('acceptance', acceptance_cases()),
    ('forks', fork_cases()), ('angle_precision', angle_precision_cases()),
    ('option_minimums', option_minimum_cases()), ('option_boundary', option_boundary_cases()),
]


class ChainSelectionTests(unittest.TestCase):
    def test_native_states_sources_and_immutable_producers(self):
        count = 0
        for family, recipes in FAMILIES:
            name = 'chain_selection_' + family
            fixture = HERE / 'fixtures' / (name + '.json')
            request = json.loads(fixture.read_bytes())
            validate_operations(request)
            self.assertEqual(request['operations'], recipes)
            path = HERE / 'observations' / (name + '.json')
            raw = (path.read_bytes() if path.exists()
                   else gzip.decompress(path.with_suffix('.json.gz').read_bytes()))
            data = json.loads(raw)
            self.assertEqual([r['id'] for r in data['results']], [o['id'] for o in recipes])
            self.assertEqual(data['provenance']['request_sha256'],
                             hashlib.sha256(fixture.read_bytes()).hexdigest())
            for file, digest in data['provenance']['sources'].items():
                frozen = gzip.decompress(
                    (HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(frozen).hexdigest(), digest, file)
            for op, row in zip(recipes, data['results'], strict=True):
                count += 1
                value = row['value']
                self.assertIs(value['source_unchanged'], True)
                self.assertEqual(value['input'], source(op['scene']))
                self.assertEqual(len(value['trace']), len(op['steps']))
                objects = len(value['input'])
                for stage, n in [('after', objects), ('redo', objects + 1), ('undo', objects)]:
                    self.assertEqual(value[stage]['objects'], n)
                    for key in ('whole', 'components'):
                        self.assertEqual(value[stage][key], value['after'][key])
                if op['id'] == 'fork_Next':
                    self.assertIn('Unknown command: Next', value['history'])
                else:
                    self.assertNotIn('Unknown command', value['history'])
        self.assertEqual(count, 144)

    def test_fixed_owned_inputs_and_isolated_worker_imports(self):
        for _, recipes in FAMILIES:
            for recipe in recipes:
                for change in [dict(seed=True), dict(seed=0.), dict(extra=True),
                               dict(scene='external'), dict(pre=[True]), dict(inspect=1)]:
                    with self.assertRaises(ValueError):
                        validate_operations(dict(operations=[dict(copy.deepcopy(recipe), **change)]))
                for invalid in (True, 0.):
                    bad = copy.deepcopy(recipe)
                    if bad['steps']:
                        bad['steps'][0]['source'] = invalid
                        with self.assertRaises(ValueError):
                            validate_operations(dict(operations=[bad]))
        request = dict(operations=[r for _, recipes in FAMILIES for r in recipes])
        with tempfile.TemporaryDirectory() as folder:
            stage_helpers(request, HERE, Path(folder))
            subprocess.run([
                sys.executable, '-I', '-c',
                'import sys;sys.path.insert(0,sys.argv[1]);'
                'import chain_selection_probe,chain_selection_driver', folder,
            ], check=True, capture_output=True)


if __name__ == '__main__':
    unittest.main()

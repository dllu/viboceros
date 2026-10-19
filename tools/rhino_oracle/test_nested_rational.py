"""Validate bounded transfer recipes and independently retained native sources."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .generate_nested_rational import request
from .nested_rational_probe import validate
from .structural_sdk import stage_helpers, validate_operations

HERE = Path(__file__).parent


class NestedRationalTransfer(unittest.TestCase):
    def recipe(self, **kw):
        op = dict(op='nested_rational_transfer', mode='create', id='case', family='box',
                  mask='mixed', gauge=1., artifact_path='/tmp/nested.3dm')
        op.update(kw)
        return op

    def test_all_capture_recipes_and_readback_modes_validate(self):
        q = request()
        self.assertEqual(len(q['operations']), 16)
        validate_operations(q)
        validate_operations(request('/tmp/rewritten'))

    def test_ambiguous_and_unbounded_sources_fail_before_launch(self):
        for kw in [dict(gauge=g) for g in (True, 0., -1., 2., float('inf'), float('nan'), '1')]+[
            dict(id='../bad'), dict(id=''), dict(mode='write'), dict(mask='unknown'),
            dict(family='surface'), dict(extra=True), dict(artifact_path='/tmp/a.json'),
            dict(mode='read')]:
            with self.subTest(kw=kw), self.assertRaises(ValueError):
                validate(self.recipe(**kw))

    def test_standalone_job_stages_a_public_sdk_probe(self):
        with tempfile.TemporaryDirectory() as folder:
            target = Path(folder)
            stage_helpers(dict(operations=[self.recipe()]), HERE, target)
            self.assertEqual([p.name for p in target.iterdir()], ['nested_rational_probe.py'])
            self.assertEqual((target/'nested_rational_probe.py').read_bytes(), (HERE/'nested_rational_probe.py').read_bytes())
            subprocess.run([sys.executable, '-I', '-c',
                'import sys;sys.path.insert(0,sys.argv[1]);import nested_rational_probe', folder], check=True)

    def test_stored_sources_exercise_every_requested_allocation_and_gauge(self):
        source = json.loads((HERE/'observations/nested_rational.json').read_text())
        q = request()
        self.assertEqual([r['id'] for r in source['results']], [o['id'] for o in q['operations']])
        for op, row in zip(q['operations'], source['results']):
            g = row['value']['geometry']
            curves = g['geometry']['segments'][:-1] if g['kind'] == 'curve' else [e['curve'] for e in g['edges']]
            self.assertEqual(len(curves), 3 if g['kind'] == 'curve' else 4 if op['family'] == 'face' else 12)
            for i, c in enumerate(curves):
                rational = op['mask'] == 'rational' or (op['mask'] == 'mixed' and i % 2 == 0)
                self.assertEqual(c['rational'], rational, op['id'])
                self.assertEqual({p['weight'] for p in c['definition']['control_points']}, {op['gauge'] if rational else 1.})
            path = Path(op['artifact_path'])
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), source['provenance']['fixtures'][path.name])

    def test_source_and_readback_use_the_same_public_probe(self):
        source = json.loads((HERE/'observations/nested_rational.json').read_text())
        readback = json.loads((HERE/'observations/nested_rational_readback.json').read_text())
        self.assertEqual(source['provenance']['sources'], readback['provenance']['sources'])
        for name, digest in source['provenance']['sources'].items():
            self.assertEqual(hashlib.sha256((HERE/name).read_bytes()).hexdigest(), digest, name)


if __name__ == '__main__':
    unittest.main()

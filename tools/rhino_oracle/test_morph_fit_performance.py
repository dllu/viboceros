"""Fitting timings retain matched inputs and independent native 3dm readback."""
import copy
import gzip
import hashlib
import json
import tempfile
from pathlib import Path
from unittest import TestCase
from .generate_morph_fit_benchmark import request as benchmark
from .generate_morph_fit_readback import request as readback
from .structural_sdk import stage_helpers, validate_operations
HERE = Path(__file__).resolve().parent


def load(stem):
    return json.loads((HERE/'observations'/(stem+'.json')).read_text())


class MorphFittingTests(TestCase):
    def test_captures_preserve_requests_sources_and_exported_file_bytes(self):
        digest = lambda b: hashlib.sha256(b).hexdigest()
        for stem, factory in [('morph_fit_benchmark', benchmark), ('morph_fit_readback', readback)]:
            path = HERE/'fixtures'/(stem+'.json')
            request = json.loads(path.read_text())
            response = load(stem)
            self.assertEqual(len(response['results']), 9)
            self.assertEqual([o['id'] for o in request['operations']], [r['id'] for r in response['results']])
            self.assertTrue(all('error' not in r for r in response['results']))
            validate_operations(request)
            generated = factory()
            for q in (request, generated):
                for op in q['operations']:
                    if 'artifact_path' in op:
                        op['artifact_path'] = Path(op['artifact_path']).name
            self.assertEqual(request, generated)
            provenance = response['provenance']
            self.assertEqual(digest(path.read_bytes()), provenance['request_sha256'])
            for name, expected in provenance['sources'].items():
                data = (HERE/name).read_bytes()
                if digest(data) != expected:
                    data = gzip.decompress((HERE/'observations/source_snapshots'/(expected+'.gz')).read_bytes())
                self.assertEqual(digest(data), expected, name)
            for name, expected in provenance.get('fixtures', {}).items():
                self.assertEqual(digest((HERE/'fixtures/morph_fit_readback'/name).read_bytes()), expected)
            with tempfile.TemporaryDirectory() as folder:
                stage_helpers(factory(), HERE, Path(folder))
                for p in Path(folder).iterdir():
                    self.assertEqual(p.read_bytes(), (HERE/p.name).read_bytes())

    def test_timing_samples_and_geometry_match_the_retained_command_cases(self):
        original = {r['id']: r['value'] for r in load('stretch')['results']}
        def near(a, b):
            if isinstance(a, list):
                self.assertEqual(len(a), len(b))
                for a, b in zip(a, b):
                    near(a, b)
            else:
                self.assertLessEqual(abs(a-b), 1e-11)
        for row in load('morph_fit_benchmark')['results']:
            value = row['value']
            samples = sorted(value['times_ns'])
            self.assertEqual(len(samples), 5)
            self.assertGreater(samples[0], 0)
            self.assertEqual(value['minimum_ns'], samples[0])
            self.assertEqual(value['median_ns'], samples[2])
            self.assertEqual(value['fit_tolerance'], 1e-6)
            self.assertEqual(len(value['result']), 1)
            near(value['source'][0]['samples'], original[row['id']]['before']['objects'][0]['geometry']['samples'])
            near(value['result'][0]['samples'], original[row['id']]['after']['objects'][0]['geometry']['samples'])

    def test_native_readback_validity_and_both_geometric_directions(self):
        for row in load('morph_fit_readback')['results']:
            v = row['value']
            self.assertTrue(v['valid'], v['diagnostics'])
            self.assertLessEqual(v['maximum_forward_distance'], 1.1e-5)
            self.assertLessEqual(v['maximum_inverse_distance'], 4.4e-5)
            if row['id'].endswith('Curve'):
                self.assertEqual(v['geometry_type'], 'NurbsCurve')
                self.assertEqual(v['forward_samples'], 257)
                self.assertEqual(v['reverse_samples'], 257)
            else:
                box = row['id'].endswith('Box')
                self.assertEqual(v['geometry_type'], 'Brep')
                self.assertEqual(v['faces'], 6 if box else 1)
                self.assertEqual(v['solid'], box)
                self.assertEqual(v['forward_samples'], 3750 if box else 625)
                self.assertGreater(v['reverse_samples'], 2400 if box else 400)
                self.assertLess(v['maximum_forward_distance'], 3e-13)
                self.assertLess(v['maximum_inverse_distance'], 3e-13)

    def test_invalid_benchmark_and_readback_recipes_fail_before_launch(self):
        for factory in (benchmark, readback):
            op = factory()['operations'][0]
            for patch in [dict(shape='Mesh'), dict(axis='Missing'), dict(id='arbitrary'), dict(extra=True)]:
                with self.subTest(patch=patch), self.assertRaises(ValueError):
                    validate_operations(dict(operations=[dict(copy.deepcopy(op), **patch)]))
        op = readback()['operations'][0]
        with self.assertRaises(ValueError):
            validate_operations(dict(operations=[dict(op, artifact_path='relative.3dm')]))

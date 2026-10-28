"""Freeze public representation transforms and their independent sweep inputs."""
import gzip
import hashlib
import json
import unittest

from .generate_sweep1_refit_representations import (
    FAMILY, HERE, INPUT_FAMILY, PROFILES, input_request, request,
)
from .generate_sweep1_refit_representation_diagnostics import (
    FAMILY as DIAGNOSTICS, request as diagnostics,
)
from .generate_sweep1_refit_clock_resolutions import (
    FAMILY as RESOLUTIONS, request as resolutions,
)
from .generate_sweep1_refit_clock_holdouts import (
    FAMILY as HOLDOUTS, request as holdouts,
)


class SweepRefitRepresentationTests(unittest.TestCase):
    def test_recipes_and_native_producers_are_frozen(self):
        for family, query, count in [(INPUT_FAMILY, input_request(), 8),
                                     (FAMILY, request(), 16),
                                     (DIAGNOSTICS, diagnostics(), 32),
                                     (RESOLUTIONS, resolutions(), 5),
                                     (HOLDOUTS, holdouts(), 5)]:
            with self.subTest(family=family):
                fixture = (HERE / ('fixtures/' + family + '.json')).read_bytes()
                observed = json.loads((HERE / ('observations/' + family + '.json')).read_bytes())
                self.assertEqual(json.loads(fixture), query)
                self.assertEqual(observed['engine'], 'rhino')
                self.assertEqual(len(observed['results']), count)
                self.assertEqual([r['id'] for r in observed['results']],
                                 [op['id'] for op in query['operations']])
                self.assertEqual(hashlib.sha256(fixture).hexdigest(),
                                 observed['provenance']['request_sha256'])
                for digest in observed['provenance']['sources'].values():
                    archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
                    self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest)

    def test_input_transforms_use_public_exact_operations(self):
        operations = input_request()['operations']
        self.assertEqual(len({op['id'] for op in operations}), 8)
        for elevated, inserted in zip(operations[::2], operations[1::2]):
            self.assertEqual(elevated['op'], 'curve_change_degree_geometry')
            self.assertEqual(elevated['degree'], 4)
            self.assertFalse(elevated['deformable'])
            self.assertEqual(inserted['op'], 'curve_insert_knot_geometry')
            self.assertEqual(inserted['parameter'], .3)
            self.assertEqual(inserted['multiplicity'], 1)
            self.assertEqual(elevated['curve'], inserted['curve'])

    def test_resolution_capture_keeps_single_and_multiple_span_controls(self):
        operations = resolutions()['operations']
        self.assertEqual(len(operations), 5)
        self.assertEqual(operations[-1]['id'], 'single-quadratic')
        self.assertEqual(len(set(operations[-1]['curve']['knots'])), 2)
        for operation in operations:
            self.assertEqual(operation['fractions'], [i / 8. for i in range(9)])
            self.assertEqual([s['fractional_tolerance'] for s in
                              operation['inspect_numerical_batch']],
                             [1e-8, 1e-6, 1e-5, 1e-4, 1e-3, 1e-2])
            self.assertTrue(all(not s['history'] for s in
                                operation['inspect_numerical_batch']))

    def test_sweep_variants_keep_the_same_refit_tolerances(self):
        profiles = json.loads((HERE / PROFILES).read_text())['profiles']
        operations = request()['operations']
        self.assertEqual(len({op['id'] for op in operations}), 16)
        for profile, start in zip(profiles, range(0, 16, 4)):
            original, chart, elevated, inserted = operations[start:start + 4]
            for operation in operations[start:start + 4]:
                self.assertEqual(operation['refit_tolerance'], profile['tolerance'])
                self.assertEqual(operation['parameters'], [0.])
                self.assertEqual(operation['rail'], original['rail'])
            self.assertEqual(original['sections'][0], profile['curve'])
            self.assertEqual(chart['sections'][0]['control_points'],
                             profile['curve']['control_points'])
            self.assertEqual(chart['sections'][0]['knots'],
                             [-7. + 20. * t for t in profile['curve']['knots']])
            self.assertEqual(elevated['sections'][0]['degree'], 4)
            self.assertEqual(inserted['sections'][0]['degree'], 3)
            self.assertIn(.3, inserted['sections'][0]['knots'])

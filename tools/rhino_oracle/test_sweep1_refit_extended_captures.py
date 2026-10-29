"""Immutable references for independent and large-tolerance section fitting."""
import gzip
import hashlib
import json
import unittest

from .generate_sweep1_refit_independent_profiles import HERE, request as independent
from .generate_sweep1_refit_large_tolerances import request as large
from .generate_sweep1_refit_large_holdouts import request as holdouts
from .generate_sweep1_refit_endpoint_frames import request as frames
from .generate_sweep1_refit_opposed_frames import request as opposed
from .generate_sweep1_refit_opposed_boundary import request as boundary
from .generate_sweep1_refit_opposed_cosines import request as cosines
from .sweep_refit_sections_probe import validate

FAMILIES = [
    ('sweep1_refit_independent_profiles', independent, 16),
    ('sweep1_refit_large_tolerances', large, 50),
    ('sweep1_refit_large_holdouts', holdouts, 16),
    ('sweep1_refit_endpoint_frames', frames, 24),
    ('sweep1_refit_opposed_frames', opposed, 18),
    ('sweep1_refit_opposed_boundary', boundary, 6),
    ('sweep1_refit_opposed_cosines', cosines, 10),
]


class ExtendedRefitCaptureTests(unittest.TestCase):
    def test_native_requests_and_producers_are_frozen(self):
        for family, build, count in FAMILIES:
            with self.subTest(family=family):
                fixture = (HERE / ('fixtures/' + family + '.json')).read_bytes()
                observed = json.loads((HERE / ('observations/' + family + '.json')).read_text())
                query = build()
                self.assertEqual(json.loads(fixture), query)
                self.assertEqual(observed['engine'], 'rhino')
                self.assertEqual(len(observed['results']), count)
                self.assertEqual(len(query['operations']), count)
                self.assertEqual(hashlib.sha256(fixture).hexdigest(),
                                 observed['provenance']['request_sha256'])
                self.assertEqual([r['id'] for r in observed['results']],
                                 [op['id'] for op in query['operations']])
                for digest in observed['provenance']['sources'].values():
                    archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
                    self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(), digest)

    def test_native_operands_and_authored_stations_are_preserved(self):
        for family, build, unused in FAMILIES:
            observed = json.loads((HERE / ('observations/' + family + '.json')).read_text())
            for operation, row in zip(build()['operations'], observed['results']):
                with self.subTest(family=family, case=operation['id']):
                    validate(operation)
                    value = row['value']
                    self.assertEqual(value['before'], value['after'])
                    self.assertEqual(value['parameters'], operation['parameters'])
                    self.assertEqual(len(value['curves']), len(operation['sections']))
                    for original, recorded in zip(operation['sections'], value['before']):
                        self.assertEqual({k: recorded[k] for k in ['degree','knots','control_points']},
                                         {k: original[k] for k in ['degree','knots','control_points']})

    def test_boundary_and_frame_controls_cover_both_contexts(self):
        for build in [independent, large, holdouts, frames, opposed, boundary, cosines]:
            operations = build()['operations']
            self.assertEqual({len(op['sections']) for op in operations}, {1, 2})
        self.assertTrue(any(op['refit_tolerance'] == 4. and 'tall' in op['id']
                            for op in holdouts()['operations']))
        self.assertTrue(any(op['sections'][0]['control_points'][0]['weight'] < 0.
                            for op in holdouts()['operations']))
        self.assertTrue(any('both-negative' in op['id'] for op in frames()['operations']))

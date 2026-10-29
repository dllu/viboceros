"""Keep public cubic diagnostics reproducible and separate from Sweep outputs."""
import copy
import gzip
import hashlib
import json
import math
from pathlib import Path
import tempfile
import unittest

from .curve_cubic_structure_probe import run
from .generate_sweep1_refit_cubic_structure import FAMILY, HERE, request
from .generate_sweep1_refit_compatibility_stages import (
    FAMILY as COMPATIBILITY, request as compatibility,
)
from .structural_sdk import stage_helpers, validate_operations


class CubicStructureTests(unittest.TestCase):
    def test_native_fixtures_and_producers_are_frozen(self):
        for family, query, count in [(FAMILY, request(), 48),
                                     (FAMILY + '_default_angle', request(math.pi / 180.), 48),
                                     (COMPATIBILITY, compatibility(), 16)]:
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

    def test_isolated_jobs_receive_the_probe_and_its_validator(self):
        query = request()
        validate_operations(query)
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory)
            stage_helpers(query, HERE, destination)
            for module in ['curve_cubic_structure_probe.py', 'geometry_validity_probe.py']:
                self.assertEqual((destination / module).read_bytes(), (HERE / module).read_bytes())

    def test_invalid_recipes_fail_before_accessing_rhino(self):
        valid = request()['operations'][0]
        changes = [dict(distance=float('nan')), dict(distance=0.), dict(angle=-1.),
                   dict(angle=float('inf')), dict(lookup_interval=[.5, .5]),
                   dict(lookup_interval=[False, 1.]), dict(unexpected=True),
                   dict(curve=dict(degree=True, knots=[], control_points=[]))]
        for update in changes:
            with self.subTest(update=update):
                operation = copy.deepcopy(valid)
                operation.update(update)
                with self.assertRaises(ValueError):
                    run(operation, {}, 1, {})

    def test_captures_retain_c1_inputs_and_valid_cubic_segments(self):
        for suffix in ['', '_default_angle']:
            observed = json.loads((HERE / ('observations/' + FAMILY + suffix + '.json')).read_text())
            for row in observed['results']:
                value = row['value']
                self.assertEqual(value['discontinuities']['C0_continuous'], [])
                self.assertEqual(value['discontinuities']['C1_continuous'], [])
                self.assertGreater(len(value['beziers']), 0)
                for piece in value['beziers']:
                    self.assertEqual(piece['degree'], 3)
                    self.assertEqual(len(piece['control_points']), 4)
                    self.assertEqual(len(piece['knots']), 8)

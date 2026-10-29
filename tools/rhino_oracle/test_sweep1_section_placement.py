"""Immutable Sweep1 placement evidence and SDK/command differences."""
from tools.verify_provenance import verify_measurement
from .qualification_sources import with_shared_sources
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import unittest
from .generate_sweep1_section_placement import request as placement
from .generate_sweep1_section_definitions import request as definitions

HERE = Path(__file__).resolve().parent


class SweepPlacementTests(unittest.TestCase):
    def test_refactor_record_binds_its_historical_and_original_sources(self):
        root = HERE.parent.parent
        record = json.loads((root / 'docs/sweep1-placement-validation.json').read_text())
        self.assertGreater(verify_measurement(record, expand_sources=with_shared_sources), 0)
        for identity, digest in record['git_sha256'].items():
            original = subprocess.check_output(['git', 'show', identity], cwd=root)
            self.assertEqual(hashlib.sha256(original).hexdigest(), digest, identity)
        self.assertTrue(record['preservation']['construction_and_parser_bytes_preserved'])
        self.assertEqual(record['automatic_placement_implemented'], False)
        self.assertEqual(record['checks']['default']['tests'], 5)
        self.assertEqual(record['checks']['native_smlib']['tests'], 5)

    def test_recipes_and_archived_producers(self):
        for family, recipe in [('sweep1_section_placement', placement),
                               ('sweep1_section_definitions', definitions)]:
            with self.subTest(family=family):
                data = (HERE / ('fixtures/' + family + '.json')).read_bytes()
                query = json.loads(data)
                native = json.loads((HERE / ('observations/' + family + '.json')).read_text())
                self.assertEqual(query, recipe())
                self.assertEqual(len(query['operations']), 14)
                self.assertEqual(native['engine'], 'rhino')
                self.assertEqual([op['id'] for op in query['operations']],
                                 [row['id'] for row in native['results']])
                self.assertEqual(hashlib.sha256(data).hexdigest(),
                                 native['provenance']['request_sha256'])
                for name, digest in native['provenance']['sources'].items():
                    archive = HERE / ('observations/source_snapshots/' + digest + '.gz')
                    self.assertEqual(hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest(),
                                     digest, name)
                for op, row in zip(query['operations'], native['results']):
                    self.assertEqual(len(row['value']), 1)
                    self.assertEqual(len(row['value'][0]['samples']), len(op['queries']))

    def test_full_control_records_distinguish_rail_end_and_reversed_order(self):
        native = json.loads((HERE / 'observations/sweep1_section_definitions.json').read_text())
        expected = {'start': (0., 5.), 'interior': (2., 5.),
                    'offset': (2., 5.), 'sloped': (1., 8.), 'two-interior': (1., 4.)}
        for row in native['results']:
            name = row['id'].removeprefix('sweep1-placement-').removesuffix('-definition')
            case, backend = name.rsplit('-', 1)
            if case == 'end':
                bounds = (5., 10.) if backend == 'sdk' else (0., 5.)
            elif case == 'two-reversed':
                bounds = (0., 5.) if backend == 'sdk' else (1., 4.)
            else:
                bounds = expected[case]
            surface = row['value'][0]['definition']
            z = [p['point'][2] for p in surface['control_points']]
            self.assertAlmostEqual(min(z), bounds[0], delta=1e-9, msg=name)
            self.assertAlmostEqual(max(z), bounds[1], delta=1e-9, msg=name)
            self.assertTrue(surface['knots_u'])
            self.assertTrue(surface['knots_v'])
            if case in ('two-interior', 'two-reversed'):
                first, last = (1., 4.) if case == 'two-interior' else (4., 1.)
                self.assertAlmostEqual(surface['control_points'][0]['point'][2], first, delta=1e-9)
                self.assertAlmostEqual(surface['control_points'][-1]['point'][2], last, delta=1e-9)


if __name__ == '__main__':
    unittest.main()

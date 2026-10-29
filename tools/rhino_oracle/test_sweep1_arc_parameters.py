"""Arc station policies, complete captured face atlases and frozen producers."""
from tools.verify_provenance import verify_measurement
from .qualification_sources import with_shared_sources
import gzip
import hashlib
import json
import math
from pathlib import Path
import unittest
from .generate_sweep1_arc_stations import request as arcs
from .generate_sweep1_refit_intervals import request as intervals
from .generate_sweep1_greville_spans import request as greville

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / 'tools/rhino_oracle'


class ArcStationTests(unittest.TestCase):
    def test_frozen_requests_preserve_every_captured_face(self):
        for family, recipe, records, faces in [('sweep1_arc_stations', arcs, 36, 40),
                                               ('sweep1_refit_intervals', intervals, 28, 35),
                                               ('sweep1_greville_spans', greville, 20, 25)]:
            with self.subTest(family=family):
                data = (HERE / ('fixtures/'+family+'.json')).read_bytes()
                request = json.loads(data)
                observed = json.loads((HERE / ('observations/'+family+'.json')).read_text())
                self.assertEqual(request, recipe())
                self.assertEqual(len(observed['results']), records)
                self.assertEqual([op['id'] for op in request['operations']], [r['id'] for r in observed['results']])
                self.assertEqual(observed['engine_version'], '8.32.26160.13001')
                self.assertEqual(hashlib.sha256(data).hexdigest(), observed['provenance']['request_sha256'])
                self.assertEqual(sum(len(r['value']) for r in observed['results']), faces)
                for name, digest in observed['provenance']['sources'].items():
                    original = gzip.decompress((HERE / ('observations/source_snapshots/'+digest+'.gz')).read_bytes())
                    self.assertEqual(hashlib.sha256(original).hexdigest(), digest, name)
                for row in observed['results']:
                    previous = None
                    for value in sorted(row['value'], key=lambda v: v['definition']['domain_u'][0]):
                        d = value['definition']
                        self.assertEqual(len(d['control_points']), math.prod(d['control_count']))
                        self.assertEqual(len(value['samples']), 81)
                        degree = d['degree'][0]
                        self.assertEqual(d['knots_u'][:degree+1], [d['domain_u'][0]]*(degree+1))
                        self.assertEqual(d['knots_u'][-degree-1:], [d['domain_u'][1]]*(degree+1))
                        if previous is not None:
                            self.assertEqual(d['domain_u'][0], previous)
                        previous = d['domain_u'][1]

    def test_refit_yes_begins_at_the_profile_even_at_a_greville_site(self):
        observed = json.loads((HERE / 'observations/sweep1_arc_stations.json').read_text())
        by_id = {r['id']:r['value'][0]['definition'] for r in observed['results']}
        start = by_id['arc-station-quarter-0.5-analytic-basis']['control_points'][0]['point']
        refit = by_id['arc-station-quarter-0.5-analytic-refit']['control_points'][0]['point']
        self.assertLess(math.dist(start,[3.,0.,0.]),1e-12)
        self.assertLess(math.dist(refit,[3/math.sqrt(2),3/math.sqrt(2),0.]),1e-12)

    def test_historical_record_binds_its_code_and_distinguishes_investigation(self):
        record = json.loads((ROOT / 'docs/sweep1-arc-parameters-validation.json').read_text())
        self.assertGreater(verify_measurement(record, expand_sources=with_shared_sources), 0)
        self.assertEqual(record['complete_image_cases'],36)
        self.assertEqual(record['calibration_cases'],1)
        self.assertEqual(record['diagnostic_records'],48)
        self.assertFalse(record['complete_rhino_sweep_compatibility'])
        continuity = record['rail_interval_regressions']
        self.assertEqual(continuity['test_functions'], 3)
        self.assertEqual(continuity['smooth_fitted_surfaces'], 12)
        self.assertTrue(continuity['source_tangents'])
        self.assertTrue(continuity['repeated_parameters_reuse_frames'])
        for name in ['crates/viboceros-geometry/src/sweep/rail.rs',
                     'crates/viboceros-geometry/tests/sweep_knot_continuity.rs']:
            self.assertIn(name, record['sha256'])


if __name__ == '__main__':
    unittest.main()

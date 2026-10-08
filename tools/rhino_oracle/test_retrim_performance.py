"""Reproducible source definitions, owned timing and unchanged geometry evidence."""
import hashlib
import json
import math
import os
from pathlib import Path
from unittest import TestCase, mock
from .client import OracleClient, OracleProtocolError, _validate_response
from .surface_retrim_profile_probe import validate_request, run

ROOT=Path(__file__).resolve().parents[2]


class RetrimPerformanceTests(TestCase):
    def request(self):
        return json.loads((ROOT/'tools/rhino_oracle/fixtures/surface_retrim_profile.json').read_text())

    def test_closed_private_idle_recipes(self):
        q=self.request();validate_request(q)
        for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(repetitions=300),dict(bounds=[[0,math.inf],[0,1]])]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            with self.assertRaisesRegex(OracleProtocolError,'private settings scheme'):OracleClient().run_rhino(q,1)
            launch.assert_not_called()

    def test_native_capture_is_complete_valid_and_does_not_mutate_sources(self):
        q=json.loads((ROOT/'tools/rhino_oracle/observations/surface_retrim_profile.json').read_text())
        _validate_response(q,'rhino',self.request())
        self.assertEqual(len(q['results']),3)
        for row in q['results']:
            v=row['value'];self.assertEqual(v['source'],v['source_after'])
            self.assertEqual(len(v['runs']),3)
            for sample in v['runs']:
                self.assertTrue(sample['valid'])
                for key in ('rebuild_ns','retrim_ns'):
                    self.assertIs(type(sample[key]),int);self.assertGreater(sample[key],0)

    def test_native_inputs_match_the_exported_rust_surfaces_and_trim_bounds(self):
        for op,suffix in zip(self.request()['operations'],('sphere','swapped','cylinder')):
            local=json.loads((ROOT/('docs/retrim-performance-'+suffix+'.json')).read_text())
            source=local['source_geometry']['faces'][0]
            surface=source['surface'];spec=op['surface']
            self.assertEqual(surface['degree'],[spec['degree_u'],spec['degree_v']])
            self.assertEqual(surface['count'],[spec['control_point_count_u'],spec['control_point_count_v']])
            self.assertEqual(surface['knots_u'],spec['knots_u']);self.assertEqual(surface['knots_v'],spec['knots_v'])
            self.assertEqual(surface['controls'],[[p['point'],p['weight']]for p in spec['control_points']])
            points=[p[0]for loop in source['loops']for trim in loop['trims']for p in trim['controls']]
            self.assertEqual(op['bounds'],[[min(p[i]for p in points),max(p[i]for p in points)]for i in (0,1)])

    def test_native_rebuilt_controls_match_the_optimized_rust_outputs(self):
        native=json.loads((ROOT/'tools/rhino_oracle/observations/surface_retrim_profile.json').read_text())
        for row,suffix in zip(native['results'],('sphere','swapped','cylinder')):
            local=json.loads((ROOT/('docs/retrim-performance-'+suffix+'.json')).read_text())
            for a,b in zip(local['runs'],row['value']['runs']):
                actual=a['geometry']['faces'][0]['surface'];expected=b['brep']['faces'][0]['definition']
                self.assertEqual(actual['degree'],expected['degree']);self.assertEqual(actual['count'],expected['control_count'])
                for (_,weight),p in zip(actual['controls'],expected['control_points']):self.assertEqual(weight,p['weight'])
                for (point,_),p in zip(actual['controls'],expected['control_points']):
                    self.assertLess(math.dist(point,p['point']),1e-6)
                for axis in ('u','v'):
                    self.assertEqual(len(actual['knots_'+axis]),len(expected['knots_'+axis]))
                    self.assertTrue(all(abs(x-y)<1e-12 for x,y in zip(actual['knots_'+axis],expected['knots_'+axis])))

    def test_profiles_retain_geometry_hashes_and_explicit_performance_gap(self):
        p=json.loads((ROOT/'docs/retrim-performance-provenance.json').read_text())
        self.assertFalse(p['full_performance_parity'])
        self.assertEqual(p['baseline_geometry_sha256'],p['optimized_geometry_sha256'])
        self.assertEqual(len(set(p['baseline_geometry_sha256'])),1)
        self.assertGreater(p['sphere_baseline_median_seconds'],p['sphere_optimized_median_seconds'])
        self.assertGreater(p['sphere_optimized_median_seconds'],p['sphere_rhino_median_seconds'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)

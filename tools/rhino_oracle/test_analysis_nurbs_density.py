"""Immutable native settings reads, rewrites, meshing and zero-density behavior."""
import gzip
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from .capture_family import HERE
from . import generate_analysis_nurbs_density as density
from . import generate_analysis_nurbs_zero as zero
from .analysis_nurbs_density_probe import ARCHIVES,validate
from .structural_sdk import stage_helpers


class AnalysisNurbsDensityTests(unittest.TestCase):
    def test_native_provenance_and_fixed_coverage(self):
        for family,generator in [('analysis_nurbs_density',density),('analysis_nurbs_zero',zero)]:
            fixture=HERE/('fixtures/'+family+'.json')
            native=json.loads((HERE/('observations/'+family+'.json')).read_text())
            query=json.loads(fixture.read_text())
            self.assertEqual(query,generator.request())
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
            self.assertEqual([r['id'] for r in native['results']],[o['id'] for o in query['operations']])
            for name,sha in native['provenance']['sources'].items():
                data=gzip.decompress((HERE/('observations/source_snapshots/'+sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha)
                if name.endswith('.3dm'):self.assertEqual(data,(HERE/name).read_bytes())
            for row in native['results']:self.assertNotIn('error',row)

    def test_native_effective_positive_densities_and_zero_profile_reset(self):
        native=json.loads((HERE/'observations/analysis_nurbs_density.json').read_text())
        for name,row in zip(ARCHIVES,native['results']):
            value=int(name[7:])/100.
            settings=row['value']['settings']
            self.assertEqual(settings['density'],value if value else .65)
            self.assertEqual(settings['subd_level'],4);self.assertTrue(settings['subd_adaptive'])
            self.assertTrue(bytes.fromhex(row['value']['archive_hex']).startswith(b'3D Geometry File Format'))
            self.assertEqual([m['source'] for m in row['value']['meshes']],['plane','sphere','cylinder'])
            self.assertEqual(row['value']['meshes'][0]['faces'],16 if value else 1)
        native=json.loads((HERE/'observations/analysis_nurbs_zero.json').read_text())['results'][0]['value']
        self.assertEqual([v['custom'] for v in native['variants']],[None,.2,.9])
        baseline=native['variants'][0]['analysis']
        self.assertEqual(baseline['RelativeTolerance'],.65)
        self.assertTrue(baseline['SimplePlanes']);self.assertFalse(baseline['ComputeCurvature'])
        for row in native['variants']:
            self.assertEqual(row['analysis'],baseline)
            if row['custom'] is not None:self.assertEqual(row['render']['RelativeTolerance'],row['custom'])

    def test_staging_copies_only_owned_bounded_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            destination=Path(directory)
            stage_helpers(density.request(),HERE,destination)
            for name in ARCHIVES:
                path=Path('fixtures/analysis_nurbs_settings')/(name+'.3dm')
                self.assertEqual((destination/path).read_bytes(),(HERE/path).read_bytes())
        base=density.request()['operations'][0]
        for change in [{'id':'../escape'},{'archive':'Other'},{'extra':True}]:
            with self.assertRaises(ValueError):validate(dict(base,**change))


if __name__=='__main__':unittest.main()

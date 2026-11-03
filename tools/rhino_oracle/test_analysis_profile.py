"""Immutable native evidence for whole-profile document migration."""
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from .capture_family import HERE
from .generate_analysis_profile import request
from .analysis_profile_probe import ARCHIVES, IDENTITY_ARCHIVES, validate
from .structural_sdk import stage_helpers


class AnalysisProfileTests(unittest.TestCase):
    def test_immutable_native_coverage_and_provenance(self):
        for identity in (False, True):
            family='analysis_profile_identity' if identity else 'analysis_profile'
            fixture=HERE/('fixtures/'+family+'.json')
            data=json.loads((HERE/('observations/'+family+'.json')).read_text())
            self.assertEqual(json.loads(fixture.read_text()), request(identity))
            self.assertEqual(data['provenance']['request_sha256'], hashlib.sha256(fixture.read_bytes()).hexdigest())
            self.assertEqual([r['id'] for r in data['results']], [o['id'] for o in request(identity)['operations']])
            for name,sha in data['provenance']['sources'].items():
                original=gzip.decompress((HERE/('observations/source_snapshots/'+sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(original).hexdigest(),sha)
                if name.endswith('.3dm'):self.assertEqual(original,(HERE/name).read_bytes())
            for row in data['results']:
                self.assertNotIn('error',row)
                for variant in row['value']['variants']:
                    self.assertEqual(variant['restored'],variant['after'])
                    if variant['read_first']:self.assertEqual(variant['before'],variant['after'])
                    else:self.assertIsNone(variant['before'])
                    self.assertTrue(bytes.fromhex(variant['archive_hex']).startswith(b'3D Geometry File Format'))

    def test_native_migration_depends_on_geometry_settings_including_subd(self):
        fast={'zero','zero-render-quality','zero-render-custom','zero-compute-off','zero-texture2','zero-custom'}
        default={'zero-default','default-compute-on','default-texture1','default-custom'}
        for family in ('analysis_profile','analysis_profile_identity'):
            data=json.loads((HERE/('observations/'+family+'.json')).read_text())
            for row in data['results']:
                name=row['id'];variants=row['value']['variants']
                a,b=[v['after'] for v in variants]
                self.assertEqual(a,b)
                density=.65 if name in fast else .8 if name in default else 1e-300 if name=='tiny' else 1e-12 if name=='epsilon' else .25 if name=='positive' else 0.
                self.assertEqual(a['RelativeTolerance'],density)
                if name in fast:
                    self.assertTrue(a['SimplePlanes']);self.assertFalse(a['ComputeCurvature'])
                    self.assertEqual(a['GridAngle'],0.);self.assertEqual(a['RefineAngle'],0.)
                if name in default:
                    self.assertFalse(a['SimplePlanes']);self.assertTrue(a['ComputeCurvature'])
                if name=='zero-subd2':self.assertEqual((a['subd_level'],a['subd_adaptive']),(2,False))
                if name=='zero-absolute4':self.assertEqual((a['subd_level'],a['subd_adaptive']),(4,False))

    def test_owned_staging_and_invalid_recipes(self):
        with tempfile.TemporaryDirectory() as directory:
            destination=Path(directory)
            stage_helpers(request(),HERE,destination)
            for name in ARCHIVES+IDENTITY_ARCHIVES:
                path=Path('fixtures/analysis_profiles')/(name+'.3dm')
                self.assertEqual((destination/path).read_bytes(),(HERE/path).read_bytes())
        op=request()['operations'][0]
        for change in ({'archive':'../escape'},{'id':'other'},{'extra':True},{'op':'analysis_profile_identity'}):
            with self.assertRaises(ValueError):validate(dict(op,**change))


if __name__=='__main__':unittest.main()

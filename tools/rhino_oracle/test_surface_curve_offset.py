"""Bounded SDK inputs and source-pinned surface-distance observations."""
import gzip,hashlib,json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .client import OracleClient,OracleProtocolError
from .surface_curve_offset_probe import request,clipping_request,validate,validate_request
from .structural_sdk import stage_helpers
HERE=Path(__file__).resolve().parent
class SurfaceCurveOffsetTests(TestCase):
    def test_qualification_preserves_the_parity_gate_and_current_source_hashes(self):
        from .report_surface_geodesic import ROOT
        r=json.loads((ROOT/'docs/surface-geodesic-qualification.json').read_text())
        self.assertEqual(r['model_epsilon'],1e-6);self.assertFalse(r['full_native_parity']);self.assertFalse(r['command_registered'])
        self.assertEqual(r['recipes'],24);self.assertEqual(r['stations'],168);self.assertEqual(r['qualified_recipes'],19);self.assertEqual(r['independent_sdk_queries'],42)
        self.assertEqual(r['unqualified_recipes'],['plane_curve_2','plane_curve_3','warped_iso_0','warped_iso_1','warped_iso_2'])
        fits=r['fitted_curves'];self.assertEqual(fits['recipes'],24);self.assertEqual(fits['curves'],23);self.assertEqual(fits['empty_recipes'],1);self.assertEqual(fits['natural_domain_rejections'],0);self.assertEqual(fits['qualified_recipes'],19);self.assertEqual(fits['unqualified_recipes'],r['unqualified_recipes']);self.assertEqual(fits['bidirectional_samples_per_curve'],130)
        clips=r['closed_gap_cases'];self.assertEqual(clips['recipes'],5);self.assertEqual(clips['qualified_recipes'],5);self.assertLess(clips['maximum_sample_difference'],1e-6)
        for path,sha in r['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha,path)
        self.assertTrue(all(x['maximum_closest_solver_difference']<1e-10 for x in r['independent_closest_points']))
    def test_fixed_sdk_evidence_and_source_closure(self):
        self.assertEqual(request(),json.loads((HERE/'fixtures/surface_curve_offset.json').read_text()))
        self.assertEqual(clipping_request(),json.loads((HERE/'fixtures/surface_curve_offset_clipping.json').read_text()))
        for label,count in [('surface_curve_offset',24),('surface_curve_offset_verify',6),('surface_curve_offset_clipping',5)]:
            path=HERE/('fixtures/'+label+'.json');q=json.loads(path.read_text());validate_request(q)
            r=json.loads((HERE/('observations/'+label+'.json')).read_text());self.assertEqual(len(r['results']),count)
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
            for name,sha in r['provenance']['sources'].items():
                data=(HERE/name).read_bytes()
                if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
            for op,row in zip(q['operations'],r['results']):
                self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row);v=row['value'];self.assertTrue(v['source_unchanged'])
                for output in v['outputs']:self.assertEqual(len(output['samples']),65);self.assertGreater(output['length'],0)
                if 'samples'in op:self.assertEqual(len(v['sample_distances']),len(op['samples']));self.assertTrue(all(d is None or d>=0 for d in v['sample_distances']))
        with tempfile.TemporaryDirectory()as folder:
            stage_helpers(request(),HERE,Path(folder));self.assertEqual((Path(folder)/'surface_curve_offset_probe.py').read_bytes(),(HERE/'surface_curve_offset_probe.py').read_bytes())
            subprocess.run([sys.executable,'-I','-c','import sys,json;sys.path.insert(0,sys.argv[1]);import surface_curve_offset_probe as p;p.validate(json.loads(sys.argv[2]))',folder,json.dumps(request()['operations'][0])],check=True,capture_output=True)
    def test_invalid_sdk_recipes_never_launch(self):
        q=request();op=q['operations'][0]
        for patch in [dict(op='other'),dict(id='../escape'),dict(case='unknown'),dict(distance=0),dict(distance=True),dict(distance=3),dict(distance=float('nan')),dict(distance=float('inf')),dict(samples=[]),dict(samples=[[True,0,0]]),dict(samples=[[0,0,float('nan')]]),dict(samples=[[0,0,1e8]]),dict(samples=[[0,0,0]]*65)]:
            with self.subTest(patch=patch),self.assertRaises(ValueError):validate(dict(op,**patch))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            for scheme,body in [(None,q),('VibocerosOracleSurfaceOffsetUnit',dict(q,iterations=2)),('VibocerosOracleSurfaceOffsetUnit',dict(q,operations=[op,op]))]:
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(launcher=Path('/bin/true'),settings_scheme=scheme).run_rhino(body,1)
            launch.assert_not_called()

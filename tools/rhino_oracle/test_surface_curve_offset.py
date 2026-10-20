"""Bounded SDK inputs and source-pinned surface-distance observations."""
import gzip,hashlib,json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .client import OracleClient,OracleProtocolError
from .surface_curve_offset_probe import request,clipping_request,extension_request,validate,validate_request
from .structural_sdk import stage_helpers
HERE=Path(__file__).resolve().parent
class SurfaceCurveOffsetTests(TestCase):
    def test_general_isocurve_independent_sdk_distance(self):
        from .surface_curve_offset_probe import general_verify_request
        path=HERE/'fixtures/surface_curve_offset_general_verify.json';q=json.loads(path.read_text())
        self.assertEqual(q,general_verify_request());validate_request(q)
        r=json.loads((HERE/'observations/surface_curve_offset_general_verify.json').read_text())
        self.assertEqual(len(r['results']),1)
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        v=r['results'][0]['value'];self.assertTrue(v['source_unchanged'])
        self.assertLess(v['sample_distances'][0],3e-7)
        self.assertGreater(v['source_first_sample_distances'][0],1e-5)
        self.assertLess(v['source_first_sample_distances'][0],1.3e-5)
    def test_general_section_sdk_captures_preserve_owned_sources_and_provenance(self):
        from .surface_curve_offset_probe import general_extension_request,validate_request
        path=HERE/'fixtures/surface_curve_offset_general_extensions.json';q=json.loads(path.read_text())
        self.assertEqual(q,general_extension_request());validate_request(q)
        r=json.loads((HERE/'observations/surface_curve_offset_general_extensions.json').read_text())
        self.assertEqual(len(r['results']),4)
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for op,row in zip(q['operations'],r['results']):
            self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
            v=row['value'];self.assertTrue(v['source_unchanged']);self.assertEqual(v['surface']['degree'],[2,2])
            self.assertIsNotNone(v['extended_source']);self.assertEqual(len(v['offset_extended_source']),1)
    def test_independent_extension_sdk_evidence_and_source_closure(self):
        path=HERE/'fixtures/surface_curve_offset_extensions.json';q=json.loads(path.read_text())
        self.assertEqual(q,extension_request());validate_request(q)
        r=json.loads((HERE/'observations/surface_curve_offset_extensions.json').read_text())
        self.assertEqual(len(r['results']),8)
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for op,row in zip(q['operations'],r['results']):
            self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
            v=row['value'];self.assertTrue(v['source_unchanged']);self.assertIsNotNone(v['extended_source'])
            self.assertGreater(len(v['offset_extended_source']),0);self.assertGreater(len(v['extended_offsets']),0)
    def test_qualification_preserves_the_parity_gate_and_current_source_hashes(self):
        from .report_surface_geodesic import ROOT
        r=json.loads((ROOT/'docs/surface-geodesic-qualification.json').read_text())
        self.assertEqual(r['model_epsilon'],1e-6);self.assertFalse(r['full_native_parity']);self.assertTrue(r['command_registered'])
        self.assertEqual(r['recipes'],24);self.assertEqual(r['stations'],168);self.assertEqual(r['qualified_recipes'],19);self.assertEqual(r['independent_sdk_queries'],42)
        self.assertEqual(r['unqualified_recipes'],['plane_curve_2','plane_curve_3','warped_iso_0','warped_iso_1','warped_iso_2'])
        fits=r['fitted_curves'];self.assertEqual(fits['recipes'],24);self.assertEqual(fits['curves'],23);self.assertEqual(fits['empty_recipes'],1);self.assertEqual(fits['natural_domain_rejections'],0);self.assertEqual(fits['qualified_recipes'],19);self.assertEqual(fits['unqualified_recipes'],r['unqualified_recipes']);self.assertEqual(fits['bidirectional_samples_per_curve'],130)
        clips=r['closed_gap_cases'];self.assertEqual(clips['recipes'],5);self.assertEqual(clips['qualified_recipes'],5);self.assertLess(clips['maximum_sample_difference'],1e-6)
        commands=r['command_cases'];self.assertEqual(commands['recipes'],8);self.assertEqual(commands['qualified_recipes'],7);self.assertEqual(commands['unqualified_recipes'],['bezier']);self.assertLess(commands['maximum_sample_difference'],4.5e-6)
        through=r['through_point_cases'];self.assertEqual(through['recipes'],16);self.assertEqual(through['qualified_recipes'],14);self.assertEqual(through['unqualified_recipes'],['through_cylinder','through_bezier'])
        selected=r['point_selection_cases'];self.assertEqual(selected['recipes'],8);self.assertEqual(selected['scripted_recipes'],6);self.assertEqual(selected['output_recipes'],5);self.assertEqual(selected['qualified_recipes'],8);self.assertLess(selected['maximum_sample_difference'],1e-6)
        curved=r['curved_end_cases'];self.assertEqual(curved['recipes'],8);self.assertEqual(curved['qualified_recipes'],5);self.assertEqual(curved['rejected_recipes'],[]);self.assertEqual(curved['unqualified_recipes'],['curved_diagonal','curved_warped','curved_warped_negative']);self.assertEqual(len(r['extension_sdk_cases']),8)
        normal=r['normal_extension_cases'];self.assertEqual(normal['recipes'],8);self.assertEqual(normal['qualified_recipes'],7);self.assertEqual(normal['unqualified_recipes'],['curved_diagonal']);self.assertLess(normal['maximum_endpoint_difference'],1e-7)
        general=r['general_section_cases'];self.assertEqual(general['recipes'],4);self.assertTrue(general['requires_native_smlib']);self.assertEqual(general['qualified_recipes'],2);self.assertEqual(general['unqualified_recipes'],['general_reversed','general_iso']);self.assertEqual(len(general['getter_cases']),1)
        sources=r['general_source_extension_cases'];self.assertEqual(sources['recipes'],4);self.assertEqual(sources['qualified_recipes'],1);self.assertLess(sources['maximum_endpoint_difference'],1e-6)
        diagonal=next(x for x in normal['cases']if x['id']=='curved_diagonal');self.assertGreater(diagonal['native_maximum_cylinder_residual'],2e-6);self.assertGreater(diagonal['native_offset_start_difference'],2.4e-3)
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

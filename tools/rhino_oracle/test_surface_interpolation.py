"""Fixed public captures and source provenance for surface interpolation."""
import copy,gzip,hashlib,json,os,tempfile
from pathlib import Path
from unittest import TestCase,mock
from . import surface_interpolate_probe as sdk,surface_interpolate_command_probe as command
from .client import OracleClient,OracleProtocolError
from .structural_sdk import stage_helpers
HERE=Path(__file__).resolve().parent
class SurfaceInterpolationTests(TestCase):
    def check_capture(self,label,factory,module,count):
        p=HERE/'fixtures'/(label+'.json');q=json.loads(p.read_text())
        self.assertEqual(q,factory());module.validate_request(q)
        r=json.loads((HERE/'observations'/(label+'.json')).read_text())
        self.assertEqual(len(r['results']),count)
        self.assertEqual(r['provenance']['request_sha256'],hashlib.sha256(p.read_bytes()).hexdigest())
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for op,row in zip(q['operations'],r['results']):
            self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
            if module is sdk:self.assertTrue(row['value']['source_unchanged'])
            else:
                v=row['value'];t=v['input_trace'];self.assertEqual(t['errors'],[]);self.assertEqual(t['completed'],t['expected'])
                outputs=[o for o in v['after_script']if o['role']=='output']
                self.assertEqual(len(outputs),0 if op['case']=='cancel'else 1)
                for o in outputs:self.assertEqual(o['layer'],'Surface interpolation output');self.assertFalse(o['selected'])
                parent=next(o for o in v['before']if o['role']=='target')
                for phase in ['after_script','undo','redo']:
                    self.assertEqual(next(o for o in v[phase]if o['role']=='target'),parent)
                if op['case']=='hole':self.assertEqual(len(v['target_definition']['topology']['edges']),8)
                if op['case'] in ['persistent','autoclose_no']:self.assertIn('Unknown command',v['history'])
        return r
    def test_sdk_fixed_inputs_and_closed_winding_evidence(self):
        self.check_capture('surface_interpolate',sdk.request,sdk,15)
        r=self.check_capture('surface_interpolate_closed',sdk.closed_request,sdk,8)
        self.assertIsNone(next(x['value']for x in r['results']if x['id']=='cylinder_closed_two')['interpolants']['uv']['curve'])
    def test_command_fixed_inputs_and_native_version_difference(self):
        self.check_capture('surface_interpolate_command',command.request,command,12)
    def test_invalid_requests_and_catalog_aliasing(self):
        for module in [sdk,command]:
            before=module.request()
            for change in ['iterations','point','case','duplicate','extra']:
                q=copy.deepcopy(before);op=q['operations'][0]
                if change=='iterations':q['iterations']=2
                elif change=='point':op['points'if module is sdk else'actions'][0][0]+=.1
                elif change=='case':op['case']='arbitrary'
                elif change=='duplicate':q['operations'][1]['id']=op['id']
                else:op['script']='arbitrary'
                with self.assertRaises(ValueError):module.validate_request(q)
            self.assertEqual(module.request(),before)
    def test_owned_captures_require_private_settings(self):
        for module in [sdk,command]:
            with mock.patch.dict(os.environ,{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as spawn:
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(module.request())
                spawn.assert_not_called()
    def test_command_helper_closure(self):
        with tempfile.TemporaryDirectory()as directory:
            dest=Path(directory);stage_helpers(command.request(),HERE,dest)
            for name in ['surface_interpolate_command_probe','surface_short_path_probe','short_path_command_probe','conic_getter_input','join_probe','merge_edges_probe','number_token','managed_mouse_callback','snap_environment']:
                self.assertEqual((dest/(name+'.py')).read_bytes(),(HERE/(name+'.py')).read_bytes())
    def test_qualification_keeps_the_native_version_gap_and_retained_sources(self):
        root=HERE.parents[1]
        r=json.loads((root/'docs/surface-interpolation-qualification.json').read_text())
        self.assertEqual(r['model_epsilon'],1e-6);self.assertFalse(r['full_native_parity'])
        self.assertEqual(r['sdk_cases']['recipes'],23)
        for name in ['command_cases','getter_cases']:
            self.assertEqual(r[name]['recipes'],12)
            self.assertIn('persistent',r[name]['unqualified_recipes'])
            self.assertIn('autoclose_no',r[name]['unqualified_recipes'])
        self.assertLessEqual(r['maximum_surface_image_bound'],5e-7)
        from tools.verify_provenance import verify
        self.assertGreaterEqual(verify(r),23)

"""Fixed inputs, private owned captures and immutable native evidence."""
import copy,gzip,hashlib,json,os,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .client import OracleClient,OracleProtocolError
from . import surface_short_path_probe as sdk,short_path_command_probe as command
from .structural_sdk import stage_helpers
HERE=Path(__file__).resolve().parent
class ShortPathTests(TestCase):
    def test_fixed_sdk_inputs_and_provenance(self):
        self.check_capture('surface_short_path',sdk,14)
    def test_fixed_command_inputs_and_provenance(self):
        self.check_capture('short_path_command',command,4)
    def check_capture(self,label,module,count):
        path=HERE/'fixtures'/(label+'.json');q=json.loads(path.read_text())
        self.assertEqual(q,module.request());module.validate_request(q)
        r=json.loads((HERE/'observations'/(label+'.json')).read_text())
        self.assertEqual(len(r['results']),count)
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
        for name,sha in r['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        for op,row in zip(q['operations'],r['results']):
            self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
            v=row['value']
            if label=='surface_short_path':
                self.assertTrue(v['source_unchanged']);self.assertIsNone(v['api_error']);self.assertIsNotNone(v['curve'])
            else:
                self.assertEqual(v['input_trace']['errors'],[])
                self.assertEqual(v['input_trace']['completed'],2)
                output=[o for o in v['after_script']if o['role']=='output']
                self.assertEqual(len(output),0 if op['case']=='cancel'else 1)
                for o in output:self.assertEqual(o['layer'],'ShortPath output')
    def test_nonfixed_endpoints_and_unbounded_requests_are_rejected(self):
        for module in [sdk,command]:
            for mutation in ['iterations','endpoint','case','duplicate','extra']:
                q=copy.deepcopy(module.request());op=q['operations'][0]
                if mutation=='iterations':q['iterations']=2
                elif mutation=='endpoint':
                    if module is sdk:op['start'][0]+=0.01
                    else:op['actions'][0][0]+=0.01
                elif mutation=='case':op['case']='arbitrary'
                elif mutation=='duplicate':q['operations'][1]['id']=op['id']
                else:op['script']='arbitrary'
                with self.assertRaises(ValueError):module.validate_request(q)
    def test_requests_do_not_alias_the_fixed_case_catalogs(self):
        for module in [sdk,command]:
            before=module.request();changed=module.request()
            if module is sdk:changed['operations'][0]['start'][0]+=0.01
            else:changed['operations'][0]['actions'][0][0]+=0.01
            with self.assertRaises(ValueError):module.validate_request(changed)
            self.assertEqual(module.request(),before)
    def test_owned_operations_require_private_settings(self):
        for module in [sdk,command]:
            with mock.patch.dict(os.environ,{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as spawn:
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(module.request())
                spawn.assert_not_called()
    def test_command_staging_contains_the_complete_helper_closure(self):
        with tempfile.TemporaryDirectory()as directory:
            destination=Path(directory);stage_helpers(command.request(),HERE,destination)
            for name in ['short_path_command_probe','surface_short_path_probe','conic_getter_input','join_probe','merge_edges_probe','number_token','managed_mouse_callback','snap_environment']:
                self.assertEqual((destination/(name+'.py')).read_bytes(),(HERE/(name+'.py')).read_bytes())
    def test_qualification_keeps_native_gaps_and_source_hashes(self):
        from .report_short_path import ROOT
        r=json.loads((ROOT/'docs/short-path-qualification.json').read_text())
        self.assertEqual(r['model_epsilon'],1e-6);self.assertFalse(r['full_native_parity']);self.assertTrue(r['command_registered'])
        self.assertEqual(r['sdk_cases']['recipes'],14);self.assertEqual(r['sdk_cases']['qualified_recipes'],4)
        self.assertEqual(r['command_cases']['recipes'],3);self.assertEqual(r['command_cases']['qualified_recipes'],1)
        self.assertEqual(r['getter_cases']['recipes'],4);self.assertEqual(r['getter_cases']['qualified_recipes'],2)
        self.assertLessEqual(r['maximum_surface_image_bound'],5e-7);self.assertGreater(r['native_cylinder_same_height_witness'],5e-4)
        for path,sha in r['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha,path)

import json,os,subprocess,sys,tempfile
from fractions import Fraction as F
from pathlib import Path
from unittest import TestCase,mock
from .curve_deviation_probe import validate
from .client import OracleClient,OracleProtocolError
from .structural_sdk import stage_helpers
class CurveDeviationTests(TestCase):
    def request(self):return json.loads(Path(__file__).with_name('fixtures').joinpath('curve_deviation.json').read_text())
    def test_captures_preserve_overlap_decisions_sources_and_complete_sdk_witnesses(self):
        q=self.request();r=json.loads(Path(__file__).with_name('observations').joinpath('curve_deviation_rhino_20261016.json').read_text());self.assertEqual(len(r['results']),18);self.assertEqual(sum(row['value']['sdk']['created']for row in r['results']),13)
        for op,row in zip(q['operations'],r['results']):
            validate(op);self.assertEqual(op['id'],row['id']);v=row['value'];self.assertTrue(v['inputs_retained']);self.assertEqual(v['created_object_count'],0);self.assertTrue(v['command_history']);self.assertEqual(v['command_returned'],v['sdk']['created'])
            self.assertEqual('Curves do not overlap.'in v['command_history'],not v['sdk']['created'])
            if v['sdk']['created']:
                for key in ['maximum','minimum']:
                    w=v['sdk'][key];self.assertEqual(len(w['parameters']),2);self.assertEqual(len(w['points']),2)
                    squared=sum((F(a)-F(b))**2 for a,b in zip(*w['points']));radius=F(w['distance']);eps=F(1e-6);self.assertLessEqual(squared,(radius+eps)**2);self.assertGreaterEqual(squared,max(F(0),radius-eps)**2)
    def test_bad_requests_and_nonprivate_launch_reject(self):
        op=self.request()['operations'][0]
        for key,value in [('curves',[]),('postselect',1),('keep_marks',1),('curves',[dict(type='point',point=[0,0,0])]*2)]:
            with self.subTest(key=key),self.assertRaises(ValueError):validate(dict(op,**{key:value}))
        with mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),mock.patch('tools.rhino_oracle.client._run_logged')as launch,self.assertRaises(OracleProtocolError):OracleClient(launcher='/bin/true').run_rhino(self.request())
        launch.assert_not_called()
    def test_standalone_helpers_import_in_the_owned_job(self):
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(self.request(),Path(__file__).parent,Path(directory));subprocess.run([sys.executable,'-I','-c',"import sys;sys.path.insert(0,sys.argv[1]);import curve_deviation_probe",directory],check=True)
    def test_command_workflow_capture_retains_circle_boundaries_and_all_permanent_marks(self):
        base=Path(__file__).parent
        q=json.loads((base/'fixtures/curve_deviation_workflows.json').read_text())
        r=json.loads((base/'observations/curve_deviation_workflows_rhino_20261016.json').read_text())
        self.assertEqual(len(r['results']),22)
        self.assertEqual(sum(row['value']['sdk']['created']for row in r['results']),16)
        for op,row in zip(q['operations'],r['results']):
            validate(op);self.assertEqual(op['id'],row['id']);v=row['value']
            self.assertTrue(v['inputs_retained']);self.assertTrue(v['command_history'])
            self.assertEqual(len(v['outputs']),v['created_object_count'])
            self.assertEqual(v['created_object_count'],(6 if op['id']=='keep-line-circle'else 12)if op['keep_marks']else 0)
            for output in v['outputs']:
                self.assertFalse(output['selected']);self.assertIsNone(output['name'])
                self.assertEqual(output['layer'],'Default');self.assertEqual(output['color_source'],'ColorFromObject')
                self.assertIn(output['color'],[[0,255,0],[255,0,0],[255,255,255]])
    def test_command_comparison_reproduces_all_failures_and_keeps_duplicate_marks(self):
        from .curve_deviation_comparison import comparison,marks_equal,displayed
        base=Path(__file__).parent
        q=json.loads((base/'fixtures/curve_deviation_workflows.json').read_text())
        local,native,expected=[json.loads((base/'observations'/('curve_deviation_workflows_'+name+'_20261016.json')).read_text())for name in ('viboceros','rhino','comparison')]
        result=comparison(q,local,native);self.assertEqual(result,expected)
        self.assertFalse(result['complete_native_command_parity_qualified'])
        self.assertEqual(result['counts']['status_matches'],22)
        self.assertEqual(result['counts']['sdk_maximum_scalar_matches'],16)
        self.assertEqual(result['counts']['command_minimum_display_matches'],15)
        self.assertLess(result['counts']['permanent_mark_sets_matching'],result['counts']['permanent_mark_cases'])
        marks=native['results'][-5]['value']['outputs']
        self.assertTrue(marks_equal(marks,list(reversed(marks))))
        self.assertFalse(marks_equal(marks,marks[:-1]))
        self.assertFalse(displayed(2.01,'Minimum deviation = 2','minimum')['within_display_rounding_and_1e_minus_6'])
        self.assertTrue(displayed(6.427876096865393,'Maximum deviation = 6.42788','maximum')['within_display_rounding_and_1e_minus_6'])
        from .curve_distance_comparison import comparison as backend_comparison
        old_q=json.loads((base/'fixtures/curve_deviation.json').read_text())
        old_local,old_native,old_expected=[json.loads((base/'observations'/('curve_deviation_'+name+'_20261016.json')).read_text())for name in ('viboceros','rhino','comparison')]
        self.assertEqual(json.loads(json.dumps(backend_comparison(old_q,old_local,old_native))),old_expected)
        for row in old_local['results']:row['value']['command_registered']=True
        self.assertTrue(backend_comparison(old_q,old_local,old_native)['command_registered'])
    def test_overlap_capture_retains_all_curved_asymmetric_and_boundary_counterexamples(self):
        base=Path(__file__).parent;total=0
        for family,count,reports in [('overlap',25,21),('endpoint_cones',17,17),('endpoint_boundary',10,10)]:
            request=json.loads((base/'fixtures'/('curve_deviation_'+family+'.json')).read_text())
            native=json.loads((base/'observations'/('curve_deviation_'+family+'_rhino_20261016.json')).read_text())
            self.assertEqual(len(request['operations']),count);self.assertEqual(len(native['results']),count)
            self.assertEqual(sum(r['value']['sdk']['created']for r in native['results']),reports);total+=count
            for op,row in zip(request['operations'],native['results']):
                validate(op);self.assertEqual(op['id'],row['id']);v=row['value']
                self.assertTrue(op['keep_marks']);self.assertTrue(v['inputs_retained']);self.assertTrue(v['command_history'])
                self.assertEqual(v['created_object_count'],len(v['outputs']))
                for o in v['outputs']:
                    self.assertFalse(o['selected']);self.assertEqual(o['color_source'],'ColorFromObject')
        self.assertEqual(total,52)
        boundary=json.loads((base/'observations/curve_deviation_endpoint_boundary_rhino_20261016.json').read_text())['results']
        self.assertGreater(boundary[1]['value']['sdk']['maximum']['distance'],2.9)
        self.assertEqual(boundary[2]['value']['sdk']['maximum']['distance'],2.)
    def test_overlap_interval_end_comparison_keeps_baseline_and_remaining_failures(self):
        from .curve_deviation_comparison import comparison
        base=Path(__file__).parent
        q=json.loads((base/'fixtures/curve_deviation_overlap.json').read_text())
        native=json.loads((base/'observations/curve_deviation_overlap_rhino_20261016.json').read_text())
        counts=[]
        for stage in ('initial_',''):
            local=json.loads((base/'observations'/('curve_deviation_overlap_'+stage+'viboceros_20261016.json')).read_text())
            expected=json.loads((base/'observations'/('curve_deviation_overlap_'+stage+'comparison_20261016.json')).read_text())
            r=comparison(q,local,native,interval_ends=True);self.assertEqual(r,expected)
            self.assertFalse(r['complete_native_command_parity_qualified']);self.assertEqual(r['cases'],25)
            counts.append(r['counts']['interval_end_sets_matching'])
            self.assertLess(r['counts']['status_matches'],25)
        self.assertEqual(counts,[9,16])

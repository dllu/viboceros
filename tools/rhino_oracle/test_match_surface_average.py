"""Own both averaging outputs and exercise the actual standalone dispatch."""
import ast,copy,json,subprocess,sys,tempfile,unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock
from . import match_surface_average_sdk as sdk,test_surface_match as owned
from .match_surface_average_cases import PILOT,request
from .structural_sdk import PROBES,stage_helpers,validate_operations

class SurfaceAverageProbeTests(unittest.TestCase):
    def test_strict_schema_and_standalone_staging(self):
        query=request();before=copy.deepcopy(query);validate_operations(query)
        for row in query['operations']:
            for changed in [dict(row,extra=True),dict(row,id='different'),dict(row,case='missing')]:
                with self.assertRaises(ValueError):sdk.validate(changed)
        with tempfile.TemporaryDirectory()as directory:
            job=Path(directory);stage_helpers(query,Path(__file__).parent,job)
            expected=PROBES['average_surface_sdk']
            self.assertEqual({p.name for p in job.iterdir()},{p+'.py'for p in [expected[0],*expected[1]]})
            subprocess.run([sys.executable,'-I','-c',
                'import sys,json;sys.path.insert(0,sys.argv[1]);import match_surface_average_sdk as p;p.validate(json.loads(sys.argv[2]))',
                str(job),json.dumps(query['operations'][0])],check=True,capture_output=True)
        self.assertEqual(query,before)
    def test_native_worker_dispatches_the_registered_operation(self):
        tree=ast.parse(Path(__file__).with_name('rhino_worker.py').read_text())
        node=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='_execute')
        namespace={};exec(compile(ast.Module(body=[node],type_ignores=[]),'worker-dispatch','exec'),namespace)
        call=mock.Mock(return_value=({'both_outputs':True},0));op=request()['operations'][0];tolerance=request()['tolerance']
        with mock.patch.dict(sys.modules,{'match_surface_average_sdk':SimpleNamespace(run=call)}):
            self.assertEqual(namespace['_execute'](op,1,tolerance),({'both_outputs':True},0))
        call.assert_called_once_with(op,namespace,1,tolerance)
    def test_average_outputs_and_failed_owned_calls_release_every_object(self):
        key=next(k for k in PILOT if k.endswith('tangency_10'));op=dict(op='average_surface_sdk',id=key,case=key)
        original=copy.deepcopy(PILOT[key])
        for stage in ['success','invalid-target','mutation','native']:
            host,sources,breps,output,create,_,setting,helper=owned.SurfaceMatchTests().host()
            target=type(output)('averaged target');create.return_value=(True,output,target)
            if stage=='invalid-target':target.IsValid=False
            elif stage=='mutation':
                def mutate(*_):breps[0].changed=True;return True,output,target
                create.side_effect=mutate
            elif stage=='native':create.side_effect=ValueError('native failure')
            with self.subTest(stage=stage),mock.patch.dict(sys.modules,{'brep_join_probe':helper}):
                if stage=='success':
                    value,_=sdk.run(op,host,1,request()['tolerance'])
                    self.assertTrue(value['source_unchanged']);self.assertIsNotNone(value['output']);self.assertIsNotNone(value['target'])
                    self.assertTrue(setting.Average);self.assertTrue(setting.ReverseMatchDirection);self.assertFalse(setting.ReverseAverageTargetDirection)
                else:
                    with self.assertRaises(ValueError):sdk.run(op,host,1,request()['tolerance'])
            for obj in sources+breps:obj.Dispose.assert_called_once()
            for obj in [output,target]:self.assertEqual(obj.Dispose.call_count,int(stage!='native'))
        self.assertEqual(PILOT[key],original)

    def test_composition_diagnostic_keeps_raw_flags_and_exposes_invalid_calls(self):
        key='average_refine_steps_rational_tangency_00';recipe=copy.deepcopy(sdk.CASES[key])
        surface=dict(degree=[3,3],control_count=[4,4],knots_u=[0.]*4+[1.]*4,
            knots_v=[0.]*4+[1.]*4,control_points=[dict(point=[i,j,0.],weight=1.)for j in range(4)for i in range(4)])
        result=lambda:dict(success=True,output=dict(surfaces=[copy.deepcopy(surface)]),target=dict(surfaces=[copy.deepcopy(surface)]))
        for failure in [False,True]:
            primary=result();responses=[(primary,7),(result(),0),(result(),0)]
            if failure:responses[1]=ValueError('invalid surface match result')
            with self.subTest(failure=failure),mock.patch.object(sdk,'run_recipe',side_effect=responses)as run:
                value,elapsed=sdk.run(dict(op='average_surface_sdk',id=key,case=key),None,1,dict())
                self.assertEqual(elapsed,7);self.assertEqual(run.call_count,2 if failure else 3)
                for call in run.call_args_list[1:]:
                    following=call.args[0]
                    self.assertTrue(following['insert_transverse_midpoints']);self.assertFalse(following['refine'])
                    self.assertTrue(following['reverse_match']);self.assertFalse(following['reverse_target'])
                    self.assertNotIn('average_pass_audit',following);self.assertEqual(len(following['sources']),2)
                audit=value['average_pass_audit']
                self.assertEqual([x['round']for x in audit],[1]if failure else[1,2])
                self.assertEqual(audit[0]['status'],'invalid'if failure else'returned')
                if failure:self.assertEqual(audit[0]['error'],'invalid surface match result')
        self.assertEqual(sdk.CASES[key],recipe)

if __name__=='__main__':unittest.main()

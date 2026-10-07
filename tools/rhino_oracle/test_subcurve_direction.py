"""Direction recipes retain real getter state, input and command completions."""
import hashlib,json,math,os,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .subcurve_direction_probe import request,validate_request,run
from .subcurve_direction_input import SubcurveDirectionPicker
from .client import OracleClient,OracleError,OracleProtocolError
ROOT=Path(__file__).resolve().parents[2]
def read(name):return json.loads((ROOT/name).read_text())
class DirectionTests(TestCase):
    def test_bounded_private_idle_recipes(self):
        q=request();validate_request(q);self.assertEqual(q,read('tools/rhino_oracle/fixtures/subcurve_direction.json'))
        for change in [dict(case='_Exit'),dict(case=[]),dict(id='x\n_Exit'),dict(extra=True)]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        for change in [dict(protocol_version=True),dict(iterations=True),dict(operations=q['operations']*2)]:
            with self.assertRaises(ValueError):validate_request(dict(q,**change))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_owned_driver_acknowledges_only_real_hover_and_never_types_commands(self):
        q=request();q['operations']=[q['operations'][2]];op=q['operations'][0];name='@subcurve-direction:'+op['id'];p=SubcurveDirectionPicker(q)
        with tempfile.TemporaryDirectory() as directory,mock.patch('tools.rhino_oracle.subcurve_direction_input.subprocess.run') as send:
            p.job=Path(directory)
            self.assertFalse(p.send_input(name,'100','200','owned'))
            self.assertEqual(len(send.call_args_list),1)
            self.assertFalse(p.send_input(name,'100','200','owned'))
            ready=p.job/('subcurve-direction-ready-'+op['id']+'.json')
            ready.write_text(json.dumps('foreign'))
            with self.assertRaises(OracleProtocolError):p.send_input(name,'100','200','owned')
            ready.write_text(json.dumps(name));self.assertTrue(p.send_input(name,'100','200','owned'))
            self.assertEqual(len(send.call_args_list),1)
            self.assertNotIn('type',send.call_args.args[0]);self.assertNotIn('key',send.call_args.args[0])
            with self.assertRaises(OracleProtocolError):p.record_diagnostics({})
            p.seen.add(name);p.record_diagnostics({})
    def test_native_direction_records_keep_motion_history_sources_and_geometry(self):
        q=read('tools/rhino_oracle/observations/subcurve_direction.json');self.assertEqual(len(q['results']),18)
        self.assertEqual([r['id'] for r in q['results']],[o['id'] for o in request()['operations']])
        self.assertEqual(sum(r['value']['success'] for r in q['results']),10)
        rows={r['value']['case']:r['value'] for r in q['results']}
        for case,v in rows.items():
            self.assertEqual(len(v['motion']),1,case);self.assertEqual(len(v['lock_prompts']),1,case)
            self.assertIn('Direction=Locked',v['lock_prompts'][0]);self.assertFalse(v['command_active'],case)
            self.assertEqual(v['after'][0]['definition'],v['before'][0]['definition'],case)
            self.assertEqual(len(v['completions']),1,case)
            self.assertEqual(v['redo'],v['after'],case)
            self.assertNotIn('Unknown command',v['history'],case)
            self.assertEqual(v['typed_inputs'][0],'Direction=Locked' if case=='direction_option' else 'D')
        for case in ('forward_point','backward_point','direction_option','mark_forward'):
            self.assertEqual(rows[case]['completions'],['Failure']);self.assertEqual(len(rows[case]['after']),1)
        for case in ('closed_forward_numeric','closed_backward_numeric','closed_circle_numeric','closed_circle_early_numeric'):
            self.assertEqual(rows[case]['completions'],['Cancel']);self.assertEqual(rows[case]['finish_prompts'],['Select curve'])
            self.assertEqual(rows[case]['typed_inputs'],['D','8',''])
        c=rows['closed_early_backward_numeric']['after'][1]
        self.assertLess(math.dist(c['samples'][0],[0.,4.8,0.]),1e-9)
        self.assertLess(math.dist(c['samples'][-1],[3.2,0.,0.]),1e-9)
        for case in ('forward_numeric','backward_numeric'):
            c=rows[case]['after'][1];self.assertAlmostEqual(math.dist(c['samples'][0],c['samples'][-1]),2.)
        self.assertEqual(len(rows['unlock_point']['free_prompts']),1)
    def test_current_producer_driver_fixture_and_raw_hashes(self):
        p=read('docs/subcurve-direction-provenance.json');self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity']);self.assertEqual(p['recipes'],18)
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)

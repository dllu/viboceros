"""Native Boolean2Objects cycle, identity, and bounded driver evidence."""
import hashlib,json,os,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .boolean_two_probe import request,validate_request,run
from .boolean_two_input import BooleanTwoPicker
from .client import OracleClient,OracleError,OracleProtocolError
ROOT=Path(__file__).resolve().parents[2]
def read(name):return json.loads((ROOT/name).read_text())
class BooleanTwoTests(TestCase):
    def test_owned_recipes_require_private_idle_empty_context(self):
        q=request();validate_request(q);self.assertEqual(q,read('tools/rhino_oracle/fixtures/boolean_two_command.json'))
        for change in [dict(case='_Exit'),dict(case=[]),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_native_cycle_order_identity_metadata_and_history(self):
        q=read('tools/rhino_oracle/observations/boolean_two_command.json');self.assertEqual(len(q['results']),11)
        expected=[15.,1.,7.,7.,14.,15.]
        for r in q['results']:
            v=r['value'];after=v['command']['after_script']
            self.assertFalse(any(o['selected'] for o in after))
            if v['case'].startswith('cycle_'):
                self.assertTrue(v['command']['success']);self.assertAlmostEqual(sum(o['volume'] for o in after),expected[v['cycles']])
                self.assertEqual(after[0]['source'],0)
                for o in after:
                    for key in ['name','layer','color','groups','attribute_text','geometry_text']:self.assertEqual(o[key],v['before'][0][key])
            if v['command']['success']:
                self.assertEqual(v['undo']['after_script'],[dict(o,selected=False) for o in v['before']]);self.assertEqual(v['redo']['after_script'],after)
            else:self.assertEqual(after,[dict(o,selected=False) for o in v['before']])
    def test_driver_never_clicks_completed_recipe_or_foreign_marker(self):
        picker=BooleanTwoPicker(request())
        with tempfile.TemporaryDirectory() as folder:
            picker.job=Path(folder);(picker.job/'worker-progress.log').write_text('PICK_DONE @boolean-two:boolean_two_disjoint\n')
            with mock.patch('tools.rhino_oracle.boolean_two_input.subprocess.run') as send:
                self.assertTrue(picker.send_input('@boolean-two:boolean_two_disjoint','100','100','owned'))
                send.assert_not_called()
                with self.assertRaises(OracleProtocolError):picker.send_input('@foreign','100','100','owned')
    def test_provenance_binds_complete_native_capture(self):
        p=read('docs/boolean-two-provenance.json');self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['recipes'],p['successful_commands'],p['retained_failures'],p['cancellations']),(11,7,3,1))
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)

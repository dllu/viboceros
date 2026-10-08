"""Owned planar Boolean2Objects recipes and bounded input routing."""
import json, os, tempfile
from pathlib import Path
from unittest import TestCase, mock
from .boolean_two_open_probe import request, validate_request, run
from .boolean_two_open_input import BooleanTwoOpenPicker
from .client import OracleClient, OracleError, OracleProtocolError
ROOT = Path(__file__).resolve().parents[2]
class BooleanTwoOpenTests(TestCase):
    def test_recipes_and_idle_private_ownership_are_required(self):
        q=request();validate_request(q)
        self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/boolean_two_open.json').read_text()))
        for change in [dict(case='_Exit'),dict(case=[]),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_completed_recipes_and_foreign_markers_never_send_input(self):
        picker=BooleanTwoOpenPicker(request())
        with tempfile.TemporaryDirectory() as folder:
            picker.job=Path(folder);(picker.job/'worker-progress.log').write_text('PICK_DONE @boolean-two:boolean_two_open_disjoint_0\n')
            with mock.patch('tools.rhino_oracle.boolean_two_open_input.subprocess.run') as send:
                self.assertTrue(picker.send_input('@boolean-two:boolean_two_open_disjoint_0','100','100','owned'))
                send.assert_not_called()
                with self.assertRaises(OracleProtocolError):picker.send_input('@foreign','100','100','owned')

    def test_native_records_and_provenance_bind_both_complete_sessions(self):
        import hashlib, copy
        combined=json.loads((ROOT/'tools/rhino_oracle/observations/boolean_two_open.json').read_text())
        p=json.loads((ROOT/'docs/boolean-two-open-provenance.json').read_text())
        self.assertEqual((p['recipes'],p['successful_commands'],p['retained_failures'],p['cancellations']),(48,46,1,1))
        self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        initial=json.loads((ROOT/p['sessions'][0]['raw']).read_text())['results']
        following=json.loads((ROOT/p['sessions'][1]['raw']).read_text())['results']
        assembled=copy.deepcopy(initial)
        for row in assembled:
            if row['value']['case'] in ('contained','disjoint','parallel'):
                row['id']+='_0';row['value']['case']+='_0'
        self.assertEqual(combined['results'],assembled+following)
        self.assertEqual({row['id']for row in combined['results']},{op['id']for op in request()['operations']})
        for row in combined['results']:
            v=row['value'];after=v['command']['after_script']
            self.assertFalse(any(o['selected']for o in after))
            if v['command']['success']:
                self.assertEqual(v['redo']['after_script'],after)
                self.assertEqual(v['undo']['after_script'],[dict(o,selected=False)for o in v['before']])
            else:self.assertEqual(after,[dict(o,selected=False)for o in v['before']])

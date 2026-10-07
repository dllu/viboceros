"""Public CreateUVCrv recipes retain sizing and projection discrepancies."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .create_uv_curves_probe import request, run, validate_request
from .client import OracleClient, OracleError, OracleProtocolError
ROOT = Path(__file__).resolve().parents[2]
def read(path): return json.loads((ROOT/path).read_text())

class CreateUvCurvesTests(TestCase):
    def test_closed_recipe_validation_and_owned_idle_context(self):
        q=request();validate_request(q)
        self.assertEqual(q,read('tools/rhino_oracle/fixtures/create_uv_curves_command.json'))
        for change in (dict(id='x\n_Exit'),dict(case=[]),dict(case='_Exit'),dict(extra=True),dict(op='script')):
            with self.subTest(change=change),self.assertRaises(ValueError):
                validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=1
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=0;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()

    def test_native_outputs_groups_history_and_off_surface_reports(self):
        cap=read('tools/rhino_oracle/observations/create_uv_curves_command.json')
        self.assertEqual(len(cap['results']),11)
        curves=points=0
        for row in cap['results']:
            v=row['value'];self.assertTrue(v['success']);self.assertFalse(v['command_active'])
            self.assertEqual(v['groups_after'],v['groups_undo'])
            original={o['source']:o for o in v['before']}
            for o in v['after']:
                if o['source'] is not None:self.assertEqual(o,dict(original[o['source']],selected=o['selected']))
                else:
                    self.assertTrue(o['selected']);self.assertEqual(len(o['groups']),1)
                    if o['kind']=='curve':curves+=1;self.assertEqual(len(o['samples']),33)
                    else:points+=1
            self.assertTrue(all(not o['selected'] for o in v['undo']))
            self.assertTrue(all(not o['selected'] for o in v['redo'] if o['source'] is not None))
            if row['id'].endswith(('off_point','off_curve')):self.assertIn('farther than 2 times absolute tolerance',v['history'])
        self.assertEqual((curves,points),(17,4))

    def test_native_length_diagnostic_exposes_coarse_sizing(self):
        rows={r['id']:r['value'] for r in read('tools/rhino_oracle/observations/create_uv_curves_command.json')['results']}
        c=rows['create_uv_cylinder']
        self.assertEqual(c['surface_size'][1],16.)
        self.assertEqual(c['iso_lengths'][0][7][1],12.566670662317494)
        self.assertGreater(c['iso_lengths'][0][7][1]-c['iso_lengths'][0][7][0],2.9e-4)
        self.assertEqual([len(x['curves']) for x in rows['create_uv_holed_plane']['trim_loops']],[4,1])

    def test_provenance_hashes_and_local_scope(self):
        p=read('docs/create-uv-curves-provenance.json')
        self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['recipes'],p['native_curves'],p['native_points']),(11,17,4))
        self.assertEqual(p['settings_scheme'],'VibocerosOracleCreateUVFinal20261006')
        for path,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest,path)

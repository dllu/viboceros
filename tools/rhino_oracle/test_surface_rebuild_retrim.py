"""Physical retrim projection, complete topology and source/history evidence."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .client import OracleClient, OracleProtocolError
from . import surface_rebuild_retrim_probe as primary
from . import surface_rebuild_retrim_followup_probe as followup

ROOT = Path(__file__).resolve().parents[2]

class SurfaceRebuildRetrimTests(TestCase):
    def test_closed_owned_idle_recipes(self):
        for probe, name in [(primary, 'surface_rebuild_retrim'), (followup, 'surface_rebuild_retrim_followup')]:
            q = probe.request(); probe.validate_request(q)
            self.assertEqual(q, json.loads((ROOT / ('tools/rhino_oracle/fixtures/' + name + '.json')).read_text()))
            for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
                with self.assertRaises(ValueError): probe.validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
            rhino = mock.Mock(); rhino.Commands.Command.InCommand.return_value = True
            with self.assertRaisesRegex(ValueError, 'idle execution'): probe.run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
            rhino.Commands.Command.InCommand.return_value = False; rhino.RhinoDoc.ActiveDoc.Objects = [object()]
            with self.assertRaisesRegex(ValueError, 'empty owned document'): probe.run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
            with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaisesRegex(OracleProtocolError,'private settings scheme'): OracleClient().run_rhino(q,1)
                launch.assert_not_called()

    def test_complete_sources_projected_outputs_history_and_provenance(self):
        p = json.loads((ROOT/'docs/surface-rebuild-retrim-provenance.json').read_text())
        self.assertFalse(p['full_native_parity'])
        for name,digest in p['sha256'].items(): self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        for capture in p['captures']:
            q = json.loads((ROOT/('tools/rhino_oracle/observations/'+capture['name']+'.json')).read_text())
            self.assertEqual(len(q['results']),capture['recipes'])
            for row in q['results']:
                v = row['value']; output = v['command']['after_script'][-1]
                self.assertTrue(v['command']['success']); self.assertFalse(v['command']['active'])
                self.assertTrue(output['valid']); self.assertEqual(v['command']['after_script'][0],v['before'][0])
                self.assertEqual(v['undo']['after_script'],v['before'])
                self.assertEqual(v['redo']['after_script'],v['command']['after_script'])
                if v['spec']['retrim']:
                    self.assertEqual(output['brep'],v['sdk_trim']['project']['brep'])
                    for axis in ('u','v'): self.assertEqual(output['definition']['domain_'+axis],v['before'][0]['definition']['domain_'+axis])
                else: self.assertEqual(output['definition'],v['rebuild_sdk'])
                for edge in output['brep']['edges']: self.assertEqual(len(edge['curve']['samples']),33)

    def test_nonuniform_uv_copy_disagrees_with_physical_projection(self):
        q = json.loads((ROOT/'tools/rhino_oracle/observations/surface_rebuild_retrim.json').read_text())
        v = next(r['value'] for r in q['results'] if r['value']['case']=='nonuniform_rectangle_retrim')
        uv = v['sdk_trim']['copy_uv']['brep']['vertices'][0]['point']
        projected = v['sdk_trim']['project']['brep']['vertices'][0]['point']
        self.assertGreater(sum((a-b)**2 for a,b in zip(uv,projected)),0.1)

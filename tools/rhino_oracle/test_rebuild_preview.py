"""Native natural-boundary evidence and owned production preview inspection."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .client import OracleClient, OracleProtocolError
from .surface_rebuild_natural_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]

class RebuildPreviewTests(TestCase):
    def test_natural_recipes_require_private_idle_empty_execution(self):
        q=request();validate_request(q)
        self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/surface_rebuild_natural.json').read_text()))
        for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            with self.assertRaisesRegex(OracleProtocolError,'private settings scheme'):OracleClient().run_rhino(q,1)
            launch.assert_not_called()

    def test_complete_native_natural_boundaries_and_independent_history(self):
        q=json.loads((ROOT/'tools/rhino_oracle/observations/surface_rebuild_natural.json').read_text())
        self.assertEqual(len(q['results']),4)
        for row in q['results']:
            v=row['value'];output=v['command']['after_script'][-1]
            self.assertTrue(v['command']['success']);self.assertFalse(v['command']['active'])
            self.assertEqual(v['undo']['after_script'],v['before'])
            self.assertEqual(v['redo']['after_script'],v['command']['after_script'])
            self.assertEqual(output['definition']['control_count'],[10,10])
            self.assertEqual(len(output['brep']['edges']),4)
            trims=output['brep']['faces'][0]['loops'][0]
            self.assertTrue(all(t['definition']['degree']==1 for t in trims))
            if v['spec']['retrim']:
                self.assertEqual(output['brep'],v['sdk_trim']['project']['brep'])
                self.assertEqual(output['definition']['domain_u'],v['before'][0]['definition']['domain_u'])
                self.assertEqual(output['definition']['domain_v'],v['before'][0]['definition']['domain_v'])

    def test_completed_production_ui_inspection_and_artifact_provenance(self):
        p=json.loads((ROOT/'docs/rebuild-preview-provenance.json').read_text())
        self.assertFalse(p['full_native_parity']);self.assertFalse(p['ui']['native_pixel_parity'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        q=json.loads((ROOT/'tools/rhino_oracle/observations/rebuild_preview_ui.json').read_text())
        self.assertTrue(q['private_xvfb']);self.assertIsNone(q['failure'])
        self.assertEqual(len(q['records']),8)
        records={r['stage']:r for r in q['records']}
        self.assertIn('Rebuilt 1',records['accepted']['ocr'])
        self.assertIn('Redid Rebuild',records['redo']['ocr'])
        for name,stage in [('docs/images/rebuild-copy-ghosted-preview.png','copy_ghosted_preview'),('docs/images/rebuild-replacement-preview.png','replacement_preview')]:
            self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),records[stage]['sha256'])

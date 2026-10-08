"""Owned TweenSurfaces source admission, cancellation and selection history."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase,mock
from .client import OracleClient,OracleError,OracleProtocolError
from .tween_surfaces_interaction_probe import request,run,validate_request
ROOT=Path(__file__).resolve().parents[2]
class TweenInteractionTests(TestCase):
    def test_bounded_private_idle_recipes(self):
        q=request();validate_request(q)
        self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/tween_surfaces_interaction.json').read_text()))
        for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_complete_selection_history_and_repeated_source_diagnostic(self):
        p=json.loads((ROOT/'docs/tween-interaction-provenance.json').read_text());q=json.loads((ROOT/'tools/rhino_oracle/observations/tween_surfaces_interaction.json').read_text())
        self.assertEqual(len(q['results']),9);self.assertFalse(p['full_native_parity'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        success=0
        for row in q['results']:
            v=row['value'];self.assertFalse(v['command']['active']);after=v['command']['after_script'];before=v['before']
            self.assertEqual([o['selected']for o in before],[i<v['spec']['pre']for i in range(2)])
            self.assertEqual([o['selected']for o in after[:2]],[o['selected']for o in before])
            if v['command']['success']:
                success+=1;self.assertEqual(len(after),4);self.assertEqual(v['undo']['after_script'],before);self.assertEqual(v['redo']['after_script'],after)
                if v['spec']['pre']==1:self.assertTrue(all(abs(p[2])<1e-12 for o in after[2:]for p in o['samples']))
            else:self.assertEqual(after,before)
        self.assertEqual(success,3)

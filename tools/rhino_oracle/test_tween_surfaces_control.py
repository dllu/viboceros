"""Owned control-matching recipes, complete native histories and provenance."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from . import tween_surfaces_control_probe as control
from . import tween_surfaces_control_initial_probe as initial
from . import tween_surfaces_control_rows_probe as rows
from . import tween_surfaces_control_followup_probe as followup
from .client import OracleClient, OracleProtocolError

ROOT = Path(__file__).resolve().parents[2]


class TweenControlTests(TestCase):
    def test_closed_recipes_require_private_idle_empty_execution(self):
        for probe, fixture in [(control, 'control'), (rows, 'control_rows'),
                               (followup, 'control_followup')]:
            q = probe.request()
            probe.validate_request(q)
            selected = json.loads((ROOT / (
                'tools/rhino_oracle/fixtures/tween_surfaces_' + fixture + '.json')).read_text())
            probe.validate_request(selected)
            self.assertTrue(all(op in q['operations'] for op in selected['operations']))
            if probe is not rows:
                self.assertEqual(q, selected)
            for change in [dict(case='_Exit'), dict(id='x\n_Exit'),
                           dict(extra=True), dict(op='script')]:
                with self.assertRaises(ValueError):
                    probe.validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
            rhino = mock.Mock()
            rhino.Commands.Command.InCommand.return_value = True
            with self.assertRaisesRegex(ValueError, 'idle execution'):
                probe.run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
            rhino.Commands.Command.InCommand.return_value = False
            rhino.RhinoDoc.ActiveDoc.Objects = [object()]
            with self.assertRaisesRegex(ValueError, 'empty owned document'):
                probe.run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
            with mock.patch.dict(os.environ, dict(DISPLAY=':301', VIBOCEROS_ORACLE_HEADLESS=':301'), clear=True), \
                    mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaisesRegex(OracleProtocolError, 'private settings scheme'):
                    OracleClient().run_rhino(q, 1)
                launch.assert_not_called()

    def test_complete_native_outputs_sources_history_and_capture_hashes(self):
        provenance = json.loads((ROOT / 'docs/tween-control-provenance.json').read_text())
        self.assertFalse(provenance['full_native_parity'])
        for name, digest in provenance['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT / name).read_bytes()).hexdigest(), digest, name)
        for capture, probe in [('initial', initial), ('followup', followup)]:
            q = json.loads((ROOT / ('tools/rhino_oracle/observations/tween_surfaces_control_' + capture + '.json')).read_text())
            self.assertEqual(q['engine_version'], provenance['engine_version'])
            self.assertEqual({r['id'] for r in q['results']}, {o['id'] for o in probe.request()['operations']})
            for row in q['results']:
                v = row['value']
                self.assertTrue(v['command']['success'])
                self.assertFalse(v['command']['active'])
                self.assertEqual(v['command']['after_script'][:2], v['before'])
                self.assertEqual(v['undo']['after_script'], v['before'])
                self.assertEqual(v['redo']['after_script'], v['command']['after_script'])
                output = v['command']['after_script'][2:]
                self.assertEqual(len(output), v['spec']['number'])
                self.assertEqual(len(v['rebuild_sdk']), 2)
                for obj in output:
                    self.assertTrue(obj['valid'])
                    self.assertEqual(len(obj['samples']), 81)
                    d = obj['definition']
                    self.assertEqual(len(d['control_points']), d['control_count'][0] * d['control_count'][1])

    def test_rejected_blend_and_row_candidates_remain_separate_diagnostics(self):
        for name, count in [('blends', 24), ('rows', 2)]:
            q = json.loads((ROOT / ('tools/rhino_oracle/observations/tween_surfaces_control_' + name + '.json')).read_text())
            self.assertEqual(len(q['results']), count)
            for row in q['results']:
                v = row['value']
                self.assertTrue(v['matched']['success'])
                self.assertTrue(v['command']['success'])
                self.assertEqual(len(v['rebuilt_blends_sdk']), v['spec']['number'])
                self.assertEqual(v['undo']['after_script'], v['before'])
                self.assertEqual(v['redo']['after_script'], v['command']['after_script'])
                self.assertEqual(len(v['command']['after_script']), 4 + v['spec']['number'])
                if name == 'rows':
                    self.assertEqual(len(v['rebuilt_control_rows']), 2)

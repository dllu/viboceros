"""Measured surface Rebuild option lifetimes and scripted degree/count coupling."""
import hashlib
import json
import os
import re
from pathlib import Path
from unittest import TestCase, mock
from .client import OracleClient, OracleProtocolError
from . import surface_rebuild_options_probe as primary
from . import surface_rebuild_option_followup_probe as followup
ROOT = Path(__file__).resolve().parents[2]

class SurfaceRebuildOptionTests(TestCase):
    def test_closed_recipes_require_private_idle_empty_execution(self):
        for probe, name in [(primary, 'surface_rebuild_options'), (followup, 'surface_rebuild_option_followup')]:
            q = probe.request(); probe.validate_request(q)
            self.assertEqual(q, json.loads((ROOT / ('tools/rhino_oracle/fixtures/' + name + '.json')).read_text()))
            for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
                with self.assertRaises(ValueError): probe.validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
            rhino = mock.Mock(); rhino.Commands.Command.InCommand.return_value = True
            with self.assertRaisesRegex(ValueError, 'idle execution'): probe.run(q['operations'][0], dict(Rhino=rhino,System=mock.Mock()))
            rhino.Commands.Command.InCommand.return_value = False; rhino.RhinoDoc.ActiveDoc.Objects = [object()]
            with self.assertRaisesRegex(ValueError, 'empty owned document'): probe.run(q['operations'][0], dict(Rhino=rhino,System=mock.Mock()))
            with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaisesRegex(OracleProtocolError, 'private settings scheme'): OracleClient().run_rhino(q,1)
                launch.assert_not_called()

    def test_complete_sequences_history_states_and_provenance(self):
        p = json.loads((ROOT / 'docs/surface-rebuild-options-provenance.json').read_text())
        states = json.loads((ROOT / 'docs/surface-rebuild-option-states.json').read_text())
        self.assertFalse(p['full_native_parity'])
        for name,digest in p['sha256'].items(): self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        for sequence in p['sequences']:
            q = json.loads((ROOT / ('tools/rhino_oracle/observations/' + sequence['name'] + '.json')).read_text())
            records = q['results'][0]['value']['records']
            self.assertEqual(len(records),sequence['steps'])
            previous = None
            for record,state in zip(records,states[sequence['name']]):
                self.assertEqual(record['step'],state['step'])
                self.assertEqual(record['accepted'],record['command']['success'])
                if previous is not None: self.assertEqual(state['initial'],previous)
                previous = state['final']
                self.assertFalse(record['command']['active'])
                if record['accepted']:
                    self.assertEqual(record['undo']['after_script'],record['before'])
                    self.assertEqual(record['redo']['after_script'],record['command']['after_script'])
                else: self.assertEqual(record['command']['after_script'],record['before'])
                for axis in (0,1): self.assertGreater(state['final']['count'][axis],state['final']['degree'][axis])
                prompts = re.findall(r'Press Enter to accept settings \( ([^)]*)\)',record['command']['history'])
                self.assertGreaterEqual(len(prompts),1)

    def test_ordered_count_and_degree_witnesses_have_distinct_measured_effects(self):
        states = json.loads((ROOT/'docs/surface-rebuild-option-states.json').read_text())
        primary = {s['step']:s for s in states['surface_rebuild_options']}
        followup = {s['step']:s for s in states['surface_rebuild_option_followup']}
        self.assertEqual(primary['count_lowers_degree']['final']['count'],[6,2])
        self.assertEqual(primary['count_lowers_degree']['final']['degree'],[2,1])
        self.assertEqual(primary['degree_raises_count']['final']['count'],[8,7])
        self.assertEqual(primary['degree_raises_count']['final']['degree'],[7,6])
        self.assertEqual(followup['lower_degree_then_count']['final']['count'],[2,2])
        self.assertEqual(followup['count_then_raise_degree']['final']['count'],[5,2])

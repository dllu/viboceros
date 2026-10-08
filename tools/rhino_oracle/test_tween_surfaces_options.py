"""Owned option lifetime capture and exact saved/default prompt evidence."""
import hashlib,json,os,re
from pathlib import Path
from unittest import TestCase,mock
from .client import OracleClient,OracleError,OracleProtocolError
from .tween_surfaces_options_probe import request,run,validate_request
ROOT=Path(__file__).resolve().parents[2]
class TweenOptionsTests(TestCase):
    def test_closed_private_idle_sequence(self):
        q=request();validate_request(q);self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/tween_surfaces_options.json').read_text()))
        for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()
    def test_complete_sequence_binds_native_prompt_defaults_and_history(self):
        p=json.loads((ROOT/'docs/tween-options-provenance.json').read_text());q=json.loads((ROOT/'tools/rhino_oracle/observations/tween_surfaces_options.json').read_text())
        self.assertEqual(p['steps'],29);self.assertFalse(p['full_native_parity']);self.assertEqual(p['immediate_fields'],['OutputLayer'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        for r in q['results'][0]['value']['records']:
            prompts=re.findall(r'Press Enter to accept options \(([^\n]*)\)',r['command']['history'])
            parse=lambda s:dict(re.findall(r'(NumberOfSurfaces|OutputLayer|MatchMethod|SampleNumber)=([A-Za-z0-9]+)',s))
            state=p['states'][r['step']];self.assertEqual(parse(prompts[0]),state['before']);self.assertEqual(parse(prompts[-1]),state['after']);self.assertEqual(r['command']['success'],state['accepted'])
            self.assertFalse(r['command']['active'])
            if r['command']['success']:self.assertEqual(r['undo']['after_script'],r['before']);self.assertEqual(r['redo']['after_script'],r['command']['after_script'])
            else:
                self.assertEqual(r['command']['after_script'][:2],r['before'])
                self.assertEqual(len(r['command']['after_script'])-2,state['output_count'])
                for obj in r['command']['after_script'][2:]:
                    self.assertIsNone(obj['source']);self.assertEqual(obj['groups'],[])
                    self.assertIn(obj['definition'],[o['definition']for o in r['before']])

    def test_inactive_sample_count_is_saved_with_an_accepted_other_method(self):
        from .tween_surfaces_sample_memory_probe import request as sample_request,validate_request as validate_sample
        request_value=sample_request();validate_sample(request_value)
        self.assertEqual(request_value,json.loads((ROOT/'tools/rhino_oracle/fixtures/tween_surfaces_sample_memory.json').read_text()))
        q=json.loads((ROOT/'tools/rhino_oracle/observations/tween_surfaces_sample_memory.json').read_text())
        p=json.loads((ROOT/'docs/tween-options-provenance.json').read_text())['inactive_sample_followup']
        self.assertEqual(len(q['results'][0]['value']['records']),8)
        for row in q['results'][0]['value']['records']:
            prompts=re.findall(r'Press Enter to accept options \(([^\n]*)\)',row['command']['history'])
            parse=lambda s:dict(re.findall(r'(NumberOfSurfaces|OutputLayer|MatchMethod|SampleNumber)=([A-Za-z0-9]+)',s))
            self.assertEqual(parse(prompts[0]),p['states'][row['step']]['before'])
            self.assertEqual(parse(prompts[-1]),p['states'][row['step']]['after'])
        self.assertEqual(p['states']['reveal_inactive_sample']['after']['SampleNumber'],'6')
        self.assertEqual(p['states']['final_defaults']['before']['SampleNumber'],'10')

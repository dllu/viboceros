"""Closed direction counts, seams/poles and corrected native chart evidence."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .client import OracleClient, OracleProtocolError
from . import surface_rebuild_closed_probe as primary
from . import surface_rebuild_closed_degrees_probe as degrees

ROOT=Path(__file__).resolve().parents[2]

class ClosedSurfaceRebuildTests(TestCase):
    def test_closed_private_idle_recipes(self):
        for probe,name in [(primary,'surface_rebuild_closed'),(degrees,'surface_rebuild_closed_degrees')]:
            q=probe.request();probe.validate_request(q)
            self.assertEqual(q,json.loads((ROOT/('tools/rhino_oracle/fixtures/'+name+'.json')).read_text()))
            for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
                with self.assertRaises(ValueError):probe.validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
            rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
            with self.assertRaisesRegex(ValueError,'idle execution'):probe.run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
            rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
            with self.assertRaisesRegex(ValueError,'empty owned document'):probe.run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
            with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
                with self.assertRaisesRegex(OracleProtocolError,'private settings scheme'):OracleClient().run_rhino(q,1)
                launch.assert_not_called()

    def test_complete_closed_counts_flags_sources_and_independent_history(self):
        p=json.loads((ROOT/'docs/closed-surface-rebuild-provenance.json').read_text())
        self.assertFalse(p['full_native_parity'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        for capture in p['captures']:
            q=json.loads((ROOT/('tools/rhino_oracle/observations/'+capture['name']+'.json')).read_text())
            self.assertEqual(len(q['results']),capture['recipes'])
            for row in q['results']:
                v=row['value'];source=v['before'][0];output=v['command']['after_script'][-1]
                self.assertTrue(v['command']['success']);self.assertFalse(v['command']['active'])
                self.assertEqual(v['command']['after_script'][0],source)
                self.assertEqual(v['undo']['after_script'],v['before'])
                self.assertEqual(v['redo']['after_script'],v['command']['after_script'])
                self.assertEqual(output['closed'],source['closed'])
                counts=[n+(d if closed else 0)for n,d,closed in zip(v['spec']['count'],v['spec']['degree'],source['closed'])]
                self.assertEqual(output['definition']['control_count'],counts)
                self.assertEqual(output['periodic'],[closed and degree>1 for closed,degree in zip(source['closed'],v['spec']['degree'])])
                self.assertEqual(output['singular'],source['singular'])
                self.assertTrue(output['valid'])

    def test_corrected_swaps_change_closed_direction_and_initial_diagnostics_stay_intact(self):
        q=json.loads((ROOT/'tools/rhino_oracle/observations/surface_rebuild_closed.json').read_text())
        old=json.loads((ROOT/'tools/rhino_oracle/observations/surface_rebuild_closed_initial.json').read_text())
        for row in q['results']:
            v=row['value']
            if '_swapped' in v['case']:
                self.assertEqual(v['before'][0]['closed'],[False,True])
                initial=next(r['value']for r in old['results']if r['value']['case']==v['case'])
                self.assertEqual(initial['before'][0]['closed'],[True,False])

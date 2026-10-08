"""Owned cylinder trim topology and native physical-projection evidence."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .client import OracleClient, OracleProtocolError
from .surface_rebuild_seam_trim_probe import request, run, validate_request
from . import surface_rebuild_crossing_hole_probe as crossing

ROOT=Path(__file__).resolve().parents[2]

class SeamTrimRebuildTests(TestCase):
    def test_closed_private_idle_recipes(self):
        q=request();validate_request(q)
        self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/surface_rebuild_seam_trim.json').read_text()))
        for change in [dict(case='_Exit'),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.assertRaises(ValueError):validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged')as launch:
            with self.assertRaisesRegex(OracleProtocolError,'private settings scheme'):OracleClient().run_rhino(q,1)
            launch.assert_not_called()

    def test_complete_topology_sources_history_and_project_api_equivalence(self):
        p=json.loads((ROOT/'docs/seam-trim-rebuild-provenance.json').read_text())
        self.assertFalse(p['full_native_parity'])
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)
        q=json.loads((ROOT/'tools/rhino_oracle/observations/surface_rebuild_seam_trim.json').read_text())
        self.assertEqual(len(q['results']),8)
        for row in q['results']:
            v=row['value'];output=v['command']['after_script'][-1]
            self.assertTrue(v['command']['success']);self.assertFalse(v['command']['active']);self.assertTrue(output['valid'])
            self.assertEqual(v['command']['after_script'][0],v['before'][0])
            self.assertEqual(v['undo']['after_script'],v['before'])
            self.assertEqual(v['redo']['after_script'],v['command']['after_script'])
            if v['spec']['retrim']:
                self.assertEqual(output['brep'],v['sdk_trim']['project']['brep'])
                for axis in ('u','v'):self.assertEqual(output['definition']['domain_'+axis],v['before'][0]['definition']['domain_'+axis])
            else:self.assertEqual(output['definition'],v['rebuild_sdk'])

    def test_ring_patch_keeps_two_uv_seam_uses_for_one_spatial_edge(self):
        q=json.loads((ROOT/'tools/rhino_oracle/observations/surface_rebuild_seam_trim.json').read_text())
        v=next(r['value']for r in q['results']if r['value']['case']=='cylinder_band_retrim')
        topology=v['command']['after_script'][-1]['brep']['topology']
        seams=[t for t in topology['faces'][0]['loops'][0]['trims']if t['type']=='Seam']
        self.assertEqual(len(seams),2);self.assertEqual(seams[0]['edge'],seams[1]['edge'])
        self.assertNotEqual(seams[0]['reversed'],seams[1]['reversed'])

    def test_crossing_hole_retains_split_seam_edges_and_one_chart_loop(self):
        q=crossing.request();crossing.validate_request(q)
        self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/surface_rebuild_crossing_hole.json').read_text()))
        native=json.loads((ROOT/'tools/rhino_oracle/observations/surface_rebuild_crossing_hole.json').read_text())
        self.assertEqual(len(native['results']),2)
        for row in native['results']:
            v=row['value'];self.assertTrue(v['command']['success'])
            self.assertEqual(v['undo']['after_script'],v['before'])
            self.assertEqual(v['redo']['after_script'],v['command']['after_script'])
            if v['spec']['retrim']:
                output=v['command']['after_script'][-1]['brep']
                self.assertEqual(output,v['sdk_trim']['project']['brep'])
                self.assertEqual(len(output['faces'][0]['loops']),1)
                self.assertEqual(len(output['faces'][0]['loops'][0]),12)
                seams=[t for t in output['topology']['faces'][0]['loops'][0]['trims']if t['type']=='Seam']
                self.assertEqual(len(seams),4)

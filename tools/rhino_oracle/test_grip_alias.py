"""Seam grip witnesses, request guards, and immutable capture provenance."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .grip_alias_probe import request, run, validate_request

ROOT = Path(__file__).resolve().parents[2]


class GripAliasTests(TestCase):
    def test_closed_schema_and_owned_document_guards(self):
        q=request()
        self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/grip_alias.json').read_text()))
        for change in (dict(id='x\n_Delete'),dict(id=[]),dict(case=[]),dict(case='_Exit'),dict(extra=True)):
            with self.subTest(change=change),self.assertRaises(ValueError):
                validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        for change in (dict(protocol_version=True),dict(iterations=True),dict(iterations=2),dict(operations=[]),dict(operations=q['operations']*2)):
            with self.subTest(change=change),self.assertRaises(ValueError):validate_request(dict(q,**change))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=1
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=0;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))

    def test_private_display_and_settings_precede_launch(self):
        for scheme,display,headless in ((None,':301',':301'),('VibocerosOracleGrips',':1',None),('VibocerosOracleGrips',':301',':302')):
            env=dict(DISPLAY=display)
            if headless:env['VIBOCEROS_ORACLE_HEADLESS']=headless
            with self.subTest(display=display,scheme=scheme),mock.patch.dict(os.environ,env,clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient(settings_scheme=scheme).run_rhino(request(),1)
                launch.assert_not_called()

    def test_native_grip_counts_and_rebuilt_circle_roundoff(self):
        r=json.loads((ROOT/'tools/rhino_oracle/observations/grip_alias.json').read_text())
        self.assertEqual([o['id'] for o in request()['operations']],[o['id'] for o in r['results']])
        self.assertEqual(r['engine_version'],'8.32.26160.13001')
        for op,row in zip(request()['operations'],r['results']):
            expected=8 if op['case']=='smoothed_circle' else 4 if op['case'] in ('outside','open') else 3
            for stage in ('before','rebuilt'):
                self.assertEqual(len(row['value'][stage]['grips']),expected)
            if op['case']=='smoothed_circle':
                after=row['value']['after'];self.assertEqual(len(after['grips']),8)
                controls=after['curve']['control_points']
                self.assertNotEqual(controls[0]['point'],controls[-1]['point'])
                self.assertTrue(after['closed'])
                self.assertEqual([e['result'] for e in row['value']['events'] if e['name']=='Smooth'],['Success'])

    def test_provenance(self):
        r=json.loads((ROOT/'docs/grip-alias-provenance.json').read_text())
        self.assertEqual(r['sdk_witnesses'],7);self.assertTrue(r['private_xvfb']);self.assertFalse(r['full_native_parity'])
        for path,expected in r['sha256'].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),expected,path)

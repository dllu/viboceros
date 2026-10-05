"""Closed native difference recipes and private-display capture evidence."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock
from .boolean_difference_probe import request, run, validate_request
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).resolve().parents[2]

class BooleanDifferenceTests(TestCase):
    def test_closed_recipes_owned_document_and_private_launch(self):
        q=request();validate_request(q)
        self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/boolean_difference_command.json').read_text()))
        for change in (dict(id='x\n_Delete'),dict(id=[]),dict(case=[]),dict(case='_Exit'),dict(extra=True)):
            with self.subTest(change=change),self.assertRaises(ValueError):
                validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        for change in (dict(protocol_version=True),dict(iterations=True),dict(iterations=2),dict(operations=[]),dict(operations=q['operations']*2),dict(operations=[q['operations'][0]]*2)):
            with self.subTest(change=change),self.assertRaises(ValueError):validate_request(dict(q,**change))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=1
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=0;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        for scheme,display,headless in ((None,':301',':301'),('VibocerosOracleDifference',':1',None),('VibocerosOracleDifference',':301',':302')):
            env=dict(DISPLAY=display)
            if headless:env['VIBOCEROS_ORACLE_HEADLESS']=headless
            with mock.patch.dict(os.environ,env,clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient(settings_scheme=scheme).run_rhino(q,1)
                launch.assert_not_called()

    def test_native_capture_event_snapshots_and_provenance(self):
        capture=json.loads((ROOT/'tools/rhino_oracle/observations/boolean_difference_command.json').read_text())
        self.assertEqual(capture['engine_version'],'8.32.26160.13001')
        self.assertEqual([o['id'] for o in request()['operations']],[r['id'] for r in capture['results']])
        rows={r['id'].removeprefix('boolean_difference_'):r['value'] for r in capture['results']}
        for case,row in rows.items():
            command=row['command'];ends=[e for e in command['events'] if e['name']=='BooleanDifference']
            self.assertEqual(len(ends),1,case);self.assertEqual(ends[0]['objects'],command['after'])
            self.assertNotIn('Unknown command:',command['history'],case)
            for output in command['after']:
                if output['source'] is None:self.assertTrue(output['valid']);self.assertTrue(output['solid'])
        self.assertAlmostEqual(rows['corner']['command']['after'][0]['volume'],7.,places=10)
        self.assertTrue(rows['pre_target']['command']['after'][0]['selected'])
        self.assertFalse(rows['corner']['command']['after'][0]['selected'])
        for output in rows['split']['command']['after']:
            if output['source'] is None:self.assertEqual(output['attribute_text'],'attribute-0');self.assertIsNone(output['geometry_text'])
        p=json.loads((ROOT/'docs/boolean-difference-provenance.json').read_text())
        self.assertEqual(p['command_witnesses'],40);self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity'])
        for path,expected in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),expected,path)

    def test_supplemental_ordering_recipes_and_coplanar_partitions(self):
        from .boolean_difference_order_probe import request, validate_request
        q=request();validate_request(q)
        self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/boolean_difference_order_command.json').read_text()))
        with self.assertRaises(ValueError):validate_request(dict(q,operations=q['operations']*2))
        r=json.loads((ROOT/'tools/rhino_oracle/observations/boolean_difference_order_command.json').read_text())
        self.assertEqual([o['id'] for o in q['operations']],[o['id'] for o in r['results']])
        for row in r['results']:
            ends=[e for e in row['value']['command']['events'] if e['name']=='BooleanDifference']
            self.assertEqual(len(ends),1);self.assertEqual(ends[0]['objects'],row['value']['command']['after'])
            self.assertEqual(ends[0]['result'],'Success')
        output=r['results'][-1]['value']['command']['after'][0]
        self.assertEqual(output['faces'],13);self.assertEqual(output['edges'],33)
        self.assertAlmostEqual(output['volume'],6.,places=10)
        p=json.loads((ROOT/'docs/boolean-difference-provenance.json').read_text());self.assertEqual(p['supplemental_witnesses'],8)
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()

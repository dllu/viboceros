"""Closed point precision recipes, owned input, exact tokens and native coordinates."""
import copy
from fractions import Fraction
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock,patch

from .client import OracleClient,OracleError,OracleProtocolError
from .point_input_precision_probe import bits,recipe,request,run,validate_request
from .point_input_precision_capture import capture

ROOT = Path(__file__).parent
PROJECT = ROOT.parents[1]


class PointInputPrecisionTests(unittest.TestCase):
    def test_closed_bounded_inputs_and_exact_tokens(self):
        q = request(); validate_request(q)
        self.assertEqual(len(q['operations']),64)
        self.assertEqual(q,json.loads((ROOT/'fixtures/point_input_precision.json').read_text()))
        for op in q['operations']:
            spec = recipe(op)
            tokens = spec['token'][1:].split(',')
            for token,value in zip(tokens,spec['point']):
                actual = float(Fraction(token)) if '/' in token else float(token)
                self.assertEqual(bits(actual),bits(value),op['id'])
        base = q['operations'][0]
        for changes in (dict(id='bad\n_Delete'),dict(id='λ'),dict(id=[]),dict(op='Delete'),
                dict(origin=[]),dict(origin='Unknown'),dict(format=[]),dict(format='Custom'),
                dict(prime=[]),dict(prime='_Delete'),dict(script='_Delete')):
            with self.subTest(changes=changes),self.assertRaises(ValueError):
                validate_request(dict(protocol_version=1,operations=[dict(base,**changes)]))
        for q in (None,[],dict(protocol_version=True,operations=[base]),
                dict(protocol_version=1,iterations=True,operations=[base]),
                dict(protocol_version=1,iterations=2,operations=[base]),
                dict(protocol_version=1,operations=[]),dict(protocol_version=1,operations=[base,base]),
                dict(protocol_version=1,operations=[dict(base,id=str(i)) for i in range(65)])):
            with self.assertRaises(ValueError): validate_request(q)

    def test_idle_empty_document_private_scheme_and_display_guards(self):
        q = request(); rhino = Mock()
        rhino.Commands.Command.InCommand.return_value = False
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):
            run(q['operations'][0],dict(Rhino=rhino,System=Mock()))
        rhino.Commands.Command.InCommand.return_value = True
        with self.assertRaisesRegex(ValueError,'idle execution'):
            run(q['operations'][0],dict(Rhino=rhino,System=Mock()))
        for scheme,display,marker in ((None,':301',':301'),('VibocerosOraclePrecision',':1',None),
                ('VibocerosOraclePrecision',':301',':302')):
            env = {'DISPLAY':display}
            if marker: env['VIBOCEROS_ORACLE_HEADLESS'] = marker
            with patch.dict(os.environ,env,clear=True),patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError,OracleProtocolError)):
                    OracleClient(settings_scheme=scheme).run_rhino(q,1)
                launch.assert_not_called()

    def test_bounded_batches_keep_raw_values_and_order(self):
        q = request(); response = dict(engine='rhino',engine_version='test',protocol_version=1,iterations=1)
        client = Mock()
        client.run_rhino.side_effect = lambda subset,timeout: dict(response,results=[dict(id=op['id'],value={'raw':True},elapsed_ns=0) for op in subset['operations']])
        with patch('tools.rhino_oracle.point_input_precision_capture.OracleClient',return_value=client) as launch,patch('builtins.print'):
            result = capture(q,'VibocerosOraclePrecision')
            self.assertEqual(launch.call_count,4)
            self.assertEqual([len(args.args[0]['operations']) for args in client.run_rhino.call_args_list],[16]*4)
            self.assertEqual([row['id'] for row in result['results']],[op['id'] for op in q['operations']])
            self.assertTrue(all(row['value']=={'raw':True} for row in result['results']))
            for size in (True,0,17):
                with self.assertRaises(ValueError): capture(q,'VibocerosOraclePrecision',batch_size=size)
            with tempfile.TemporaryDirectory() as folder:
                client.run_rhino.reset_mock()
                saved = capture(q,'VibocerosOraclePrecision',checkpoint_dir=folder)
                self.assertEqual(client.run_rhino.call_count,4)
                self.assertEqual(capture(q,'VibocerosOraclePrecision',checkpoint_dir=folder),saved)
                self.assertEqual(client.run_rhino.call_count,4)
                changed = copy.deepcopy(q); changed['operations'][0]['prime'] = 'ScalePositions'
                with self.assertRaisesRegex(OracleProtocolError,'checkpoint'):
                    capture(changed,'VibocerosOraclePrecision',checkpoint_dir=folder)
                path = Path(folder)/'000.json'; checkpoint = json.loads(path.read_text())
                checkpoint['source_sha256']['point_input_precision_probe.py'] = 'stale'
                path.write_text(json.dumps(checkpoint))
                with self.assertRaisesRegex(OracleProtocolError,'checkpoint'):
                    capture(q,'VibocerosOraclePrecision',checkpoint_dir=folder)

    def test_native_point_coordinates_preserve_exact_neighbor_bits(self):
        observed = json.loads((ROOT/'observations/point_input_precision.json').read_text())
        q = request()
        self.assertEqual(observed['engine'],'rhino')
        self.assertEqual(observed['engine_version'],'8.32.26160.13001')
        self.assertEqual(observed['protocol_version'],1)
        self.assertEqual(observed['iterations'],1)
        self.assertEqual([row['id'] for row in observed['results']],[op['id'] for op in q['operations']])
        xbits = dict(Zero='0000000000000000',FloatBelow='3e7fffffffffffff',
                     FloatAt='3e80000000000000',FloatAbove='3e80000000000001',
                     FloatHigh='3e80000100000000',NegativeAbove='be80000000000001')
        by_origin = {}
        for op,row in zip(q['operations'],observed['results']):
            with self.subTest(id=op['id']):
                value = row['value']; spec = recipe(op)
                expected = [xbits.get(op['origin'],'0000000000000000'),'0000000000000000','0000000000000000']
                if op['origin']=='YAbove': expected[1] = '3e80000000000001'
                if op['origin']=='ZAbove': expected[2] = '3e80000000000001'
                self.assertEqual(value['recipe'],spec)
                self.assertTrue(value['success'])
                self.assertEqual(len(value['after']),1)
                coordinate = value['after'][0]
                self.assertEqual(coordinate['bits'],expected)
                self.assertEqual([bits(x) for x in coordinate['point']],expected)
                self.assertEqual(coordinate['tiny'],all(abs(x)<=2.**-23 for x in spec['point']))
                self.assertEqual(value['after_script'],value['after'])
                point_events = [event for event in value['events'] if event['name']=='Point']
                self.assertEqual(len(point_events),1)
                self.assertEqual(point_events[0]['result'],'Success')
                self.assertEqual(point_events[0]['objects'],value['after'])
                self.assertEqual(point_events[0]['selected'],[])
                self.assertIn(spec['token'],value['history'])
                self.assertEqual(value['tolerance'],1e-9)
                by_origin.setdefault(op['origin'],[]).append(coordinate)
        self.assertEqual(len(by_origin),8)
        for coordinates in by_origin.values():
            self.assertEqual(len(coordinates),8)
            self.assertTrue(all(p==coordinates[0] for p in coordinates))

    def test_native_batches_and_provenance_match_retained_artifacts(self):
        observed = json.loads((ROOT/'observations/point_input_precision.json').read_text())
        manifest = json.loads((PROJECT/'docs/point-input-precision-provenance.json').read_text())
        self.assertTrue(manifest['private_xvfb'])
        self.assertFalse(manifest['full_native_parity'])
        self.assertEqual(manifest['native_point_recipes'],64)
        self.assertEqual(manifest['positive_command_replays'],64)
        self.assertEqual(manifest['positive_application_replays'],128)
        self.assertEqual(manifest['coordinate_comparison'],'exact_binary64_bits')
        self.assertEqual(manifest['engine_version'],observed['engine_version'])
        batches = observed['capture_batches']
        self.assertEqual([len(batch['ids']) for batch in batches],[16]*4)
        self.assertEqual([name for batch in batches for name in batch['ids']],
                         [op['id'] for op in request()['operations']])
        self.assertEqual(manifest['settings_schemes'],[batch['settings_scheme'] for batch in batches])
        self.assertTrue(all(batch['settings_scheme'].startswith('VibocerosOracle') for batch in batches))
        for name,expected in manifest['sha256'].items():
            self.assertEqual(hashlib.sha256((PROJECT/name).read_bytes()).hexdigest(),expected,name)
        self.assertEqual(manifest['unresolved_getpoint_diagnostic']['id'],'point-input-precision-48')
        self.assertFalse(manifest['unresolved_getpoint_diagnostic']['positive_replay'])


if __name__=='__main__': unittest.main()

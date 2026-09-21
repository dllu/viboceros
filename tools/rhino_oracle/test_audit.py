"""Errors stay errors; complete replay compares all successful raw records."""
import copy
import json
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import patch

from .audit import compare_audit_response, validate_audit_response
from .client import OracleClient, OracleError, OracleProtocolError, compare_responses
from . import __main__ as cli


def records():
    header = dict(protocol_version=1, engine_version="test", iterations=1)
    values = [dict(id=name, value=value, elapsed_ns=100) for name, value in
              [('first', {'point': [1., 2., 3.]}), ('failed', 9), ('last', [7., 8.])]]
    reference = dict(header, engine="rhino", results=copy.deepcopy(values))
    outcomes = [dict(status="success", result=values[0]),
                dict(status="failure", id="failed", error=dict(kind="command", message="bounded join rejected")),
                dict(status="success", result=values[2])]
    return dict(header, engine="viboceros", outcomes=outcomes), reference


def request():
    return dict(protocol_version=1, operations=[dict(op="point_distance", id=i) for i in ('first', 'failed', 'last')])


class AuditTests(unittest.TestCase):
    def test_successes_survive_failure_without_invented_geometry_or_zero_error(self):
        native, reference = records()
        before = copy.deepcopy((native, reference))
        report = compare_audit_response(native, reference)
        encoded = report.as_dict()
        self.assertEqual((report.matched, report.native_failed, encoded['mismatched']), (2, 1, 0))
        self.assertFalse(report.passed)
        self.assertEqual([o.id for o in report.operations], ['first', 'failed', 'last'])
        row = encoded['operations'][1]
        self.assertIsNone(row['comparison'])
        self.assertEqual(row['native_error'], dict(kind='command', message='bounded join rejected'))
        self.assertNotIn('max_absolute_error', row)
        self.assertNotIn('elapsed_ns', row)
        self.assertEqual((native, reference), before)
        json.dumps(encoded, allow_nan=False)
        with self.assertRaises(OracleProtocolError):
            compare_responses(native, reference)

    def test_numeric_and_structural_differences_are_still_compared_after_an_error(self):
        native, reference = records()
        reference['results'].reverse()
        reference['results'][0]['value'] = [7., 8.1, 9.]
        report = compare_audit_response(native, reference, 1e-10, 1e-12)
        self.assertEqual((report.matched, report.native_failed, report.as_dict()['mismatched']), (1, 1, 1))
        last = report.operations[-1].comparison
        self.assertFalse(last.passed)
        self.assertGreater(last.max_absolute_error, .09)
        self.assertEqual(len(last.differences), 2)

    def test_all_failed_and_empty_audits_do_not_miscount(self):
        native, reference = records()
        native['outcomes'] = [native['outcomes'][1]]
        reference['results'] = [reference['results'][1]]
        report = compare_audit_response(native, reference)
        self.assertEqual((report.matched, report.native_failed, report.passed), (0, 1, False))
        native['outcomes'] = []
        reference['results'] = []
        self.assertTrue(compare_audit_response(native, reference).passed)

    def test_rejects_malformed_or_contradictory_outcomes_and_nonfinite_successes(self):
        native, _ = records()
        bad = []
        for key, value in [('protocol_version', True), ('iterations', False), ('engine', 'rhino'),
                           ('outcomes', None), ('error', 'crashed'), ('results', [])]:
            candidate = copy.deepcopy(native); candidate[key] = value; bad.append(candidate)
        for outcome in [None, {}, {'status':'unknown'},
                        dict(status='failure', id='failed', error={'kind':'geometry','message':''}),
                        dict(status='failure', id='failed', error={'kind':[],'message':'bad'}),
                        dict(status='failure', id='failed', error={'kind':'unknown','message':'bad'}),
                        dict(status='failure', id='failed', error={'kind':'io','message':'missing'}, value=None),
                        dict(status='success', result={'id':'last','value':0.,'elapsed_ns':0}),
                        dict(status='success', result={'id':'failed','value':float('nan'),'elapsed_ns':0}),
                        dict(status='success', result={'id':'failed','value':0.,'elapsed_ns':0,'error':'bad'}),
                        dict(status='success', result={'id':'failed','value':0.,'elapsed_ns':False})]:
            candidate = copy.deepcopy(native); candidate['outcomes'][1] = outcome; bad.append(candidate)
        for candidate in bad:
            with self.subTest(candidate=candidate), self.assertRaises(OracleProtocolError):
                validate_audit_response(candidate)

    def test_replay_preflight_does_not_launch_for_bad_ids_metadata_or_epsilon(self):
        native, reference = records()
        client = OracleClient()
        bad = []
        for key,value in [('protocol_version', True), ('iterations', 2), ('iterations', True),
                          ('operations', None), ('operations', [{'id':'missing'}]),
                          ('operations', [{'id':'first'}, {'id':'first'}])]:
            candidate=request();candidate[key]=value;bad.append(candidate)
        with patch.object(client, 'run_viboceros_audit', return_value=native) as launch:
            for candidate in bad:
                with self.subTest(candidate=candidate), self.assertRaises(OracleProtocolError):
                    client.replay(candidate, reference)
            for epsilon in [-1., float('nan'), float('inf'), True]:
                with self.assertRaises(OracleProtocolError):
                    client.replay(request(), reference, absolute_epsilon=epsilon)
            launch.assert_not_called()
            self.assertEqual(client.replay(request(), reference).matched, 2)
            launch.assert_called_once()

    def test_process_and_reference_errors_remain_fatal(self):
        native, reference = records()
        client=OracleClient()
        with patch.object(client, 'run_viboceros_audit', side_effect=OracleError('process died')) as launch:
            with self.assertRaisesRegex(OracleError, 'process died'):
                client.replay(request(), reference)
            self.assertEqual(launch.call_count, 1)
        for key,value in [('iterations', 2), ('protocol_version', True), ('error', 'Rhino failed')]:
            bad=copy.deepcopy(reference);bad[key]=value
            with self.assertRaises(OracleError):compare_audit_response(native,bad)
        reference['results'].pop()
        with self.assertRaisesRegex(OracleProtocolError, 'ids'):
            compare_audit_response(native, reference)

    def test_native_audit_uses_one_process_and_cleans_its_owned_files(self):
        native,_=records();seen=[];client=OracleClient()
        def run(command,cwd,timeout):
            self.assertIn('--audit',command)
            seen.append(Path(command[-1]))
            Path(command[-1]).write_text(json.dumps(native),encoding='utf-8')
            return types.SimpleNamespace(returncode=0,stdout='',stderr='')
        with patch('tools.rhino_oracle.client._run',side_effect=run) as launch:
            self.assertEqual(client.run_viboceros_audit(request()),native)
            launch.assert_called_once()
        self.assertFalse(seen[0].parent.exists())

    def test_replay_owns_exports_and_cleans_them_after_success_or_process_failure(self):
        native,reference=records()
        fixture=request()
        fixture['operations'][0]=dict(op='join_command',id='first',sources=[
            dict(brep=dict(source=dict(type='box'),artifact_path='/unowned/input.3dm'))])
        before=copy.deepcopy(fixture)
        for fail in [False,True]:
            paths=[]
            def run(prepared,timeout):
                path=Path(prepared['operations'][0]['sources'][0]['brep']['artifact_path'])
                self.assertNotEqual(path,Path('/unowned/input.3dm'))
                self.assertFalse(path.exists())
                paths.append(path);path.write_bytes(b'owned export')
                if fail:raise OracleError('process failure')
                return native
            client=OracleClient()
            with patch.object(client,'run_viboceros_audit',side_effect=run) as launch:
                if fail:
                    with self.assertRaisesRegex(OracleError,'process failure'):client.replay(fixture,reference)
                else:self.assertEqual(client.replay(fixture,reference).matched,2)
                launch.assert_called_once()
            self.assertEqual(fixture,before)
            self.assertFalse(paths[0].parent.exists())

    def test_malformed_artifact_requests_are_protocol_errors_before_launch(self):
        _,reference=records();client=OracleClient()
        bad_operations=[dict(op='join_command'),dict(op='join_command',sources=None),
                        dict(op='join_command',sources=[None]),
                        dict(op='join_command',sources=[{'brep':'not an object'}]),
                        dict(op='brep_join',sources='bad'),dict(op='border_command',source=None)]
        with patch.object(client,'run_viboceros_audit') as launch:
            for operation in bad_operations:
                fixture=request();fixture['operations'][0]=dict(operation,id='first')
                with self.subTest(operation=operation),self.assertRaises(OracleProtocolError):
                    client.replay(fixture,reference)
            launch.assert_not_called()

    def test_cli_replay_requires_observations_and_returns_failure_for_native_errors(self):
        native,reference=records()
        with tempfile.TemporaryDirectory() as directory:
            fixture=Path(directory)/'fixture.json';observation=Path(directory)/'rhino.json'
            fixture.write_text(json.dumps(request()),encoding='utf-8')
            observation.write_text(json.dumps(reference),encoding='utf-8')
            with patch('sys.argv',['oracle','replay',str(fixture),'--observations',str(observation)]), \
                    patch.object(OracleClient,'run_viboceros_audit',return_value=native) as launch, \
                    patch('builtins.print') as output:
                self.assertEqual(cli.main(),1)
                self.assertEqual(json.loads(output.call_args.args[0])['native_failed'],1)
                launch.assert_called_once()
            with patch('sys.argv',['oracle','replay',str(fixture)]), patch('sys.stderr'), \
                    self.assertRaises(SystemExit) as error:
                cli.main()
            self.assertEqual(error.exception.code,2)

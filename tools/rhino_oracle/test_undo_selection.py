"""Validation and cleanup contracts for the isolated undo-selection worker."""
import ast
import copy
import json
import os
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock
from .client import OracleProtocolError
from .undo_selection import validate_request

def worker_functions(scope, *names):
    tree=ast.parse(Path(__file__).with_name('undo_selection_worker.py').read_text())
    functions=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in names]
    assert len(functions)==len(names)
    exec(compile(ast.Module(body=functions,type_ignores=[]),'worker','exec'),scope)
    return scope

class UndoSelectionTests(unittest.TestCase):
    def test_fixture_and_rejected_requests(self):
        request=json.loads(Path(__file__).with_name('fixtures').joinpath('undo_selection.json').read_text())
        validate_request(request)
        for changes in [dict(kind='Other'),dict(clear=1),dict(op='group_picking'),dict(id='bad\ncase')]:
            invalid=copy.deepcopy(request);invalid['operations'][0].update(changes)
            with self.assertRaises(OracleProtocolError): validate_request(invalid)
        for changes in [dict(iterations=True),dict(iterations=2),dict(protocol_version=True),dict(operations=[]),dict(operations=request['operations']*2)]:
            with self.assertRaises(OracleProtocolError): validate_request(dict(request,**changes))

    def test_cleanup_attempts_every_owned_object_after_failure(self):
        tree=ast.parse(Path(__file__).with_name('undo_selection_worker.py').read_text())
        function=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='cleanup')
        objects=Mock()
        objects.FindId.return_value=object()
        objects.Delete.side_effect=RuntimeError('failed deletion')
        state={'owned':set([1,2,3])}
        scope={'d':SimpleNamespace(Objects=objects),'state':state}
        exec(compile(ast.Module(body=[function],type_ignores=[]),'worker','exec'),scope)
        with self.assertRaises(ValueError): scope['cleanup']()
        self.assertEqual(objects.Delete.call_count,3)
        self.assertEqual(state['owned'],set())

    def test_deselection_failure_does_not_skip_owned_object_cleanup(self):
        objects=Mock()
        objects.UnselectAll.side_effect=RuntimeError('deselect failed')
        objects.FindId.return_value=object()
        objects.Delete.return_value=True
        state={'owned':set([1,2])}
        scope=worker_functions({'d':SimpleNamespace(Objects=objects),'state':state},'cleanup')
        with self.assertRaisesRegex(ValueError,'deselect failed'): scope['cleanup']()
        self.assertEqual(objects.Delete.call_count,2)
        self.assertEqual(state['owned'],set())

    def test_finalization_publishes_scan_failure_and_still_cleans_registered_objects(self):
        for baseline in [None,set([99])]:
            with self.subTest(baseline=baseline), tempfile.TemporaryDirectory() as root:
                class Event:
                    def __isub__(self, callback): return self
                app=SimpleNamespace(Idle=Event(),Version='test',RunScript=Mock(side_effect=RuntimeError('exit failed')))
                table=Mock()
                table.FindId.return_value=object()
                table.Delete.return_value=True
                scan=Mock(side_effect=RuntimeError('scan failed'))
                state={'done':False,'baseline':baseline,'owned':set([1]),'results':[{'partial':True}]}
                scope=worker_functions(dict(state=state,root=root,os=os,json=json,objects=scan,
                    d=SimpleNamespace(Objects=table),Rhino=SimpleNamespace(RhinoApp=app),idle=object()),'cleanup','finish')
                scope['finish']('original failure')
                scope['finish']('must not finalize twice')
                response=json.loads(Path(root,'response.json').read_text())
                self.assertEqual(response['results'],[])
                self.assertIn('original failure',response['error'])
                self.assertEqual('scan failed' in response['error'],baseline is not None)
                self.assertEqual(scan.call_count,int(baseline is not None))
                table.Delete.assert_called_once_with(1,True)
                app.RunScript.assert_called_once_with('_Exit _No',False)
                self.assertFalse(Path(root,'response.json.tmp').exists())

    def test_failed_command_and_empty_insertion_are_rejected(self):
        app=SimpleNamespace(RunScript=Mock(return_value=False))
        state={'owned':set()}
        scope=worker_functions(dict(state=state,Rhino=SimpleNamespace(RhinoApp=app),
            System=SimpleNamespace(Guid=SimpleNamespace(Empty=0))),'run_command','own')
        with self.assertRaisesRegex(ValueError,'command failed: _Undo'): scope['run_command']('_Undo')
        with self.assertRaisesRegex(ValueError,'insertion failed'): scope['own'](0)
        self.assertEqual(state['owned'],set())
        self.assertEqual(scope['own'](17),17)
        self.assertEqual(state['owned'],set([17]))

    def test_finalization_discovers_outputs_without_deleting_baseline_objects(self):
        with tempfile.TemporaryDirectory() as root:
            class Event:
                def __isub__(self, callback): return self
            app=SimpleNamespace(Idle=Event(),Version='test',RunScript=Mock(return_value=True))
            table=Mock()
            table.FindId.return_value=object()
            table.Delete.return_value=True
            state={'done':False,'baseline':set([99]),'owned':set([1]),'results':[]}
            scope=worker_functions(dict(state=state,root=root,os=os,json=json,
                objects=lambda:[SimpleNamespace(Id=99),SimpleNamespace(Id=2)],
                d=SimpleNamespace(Objects=table),Rhino=SimpleNamespace(RhinoApp=app),idle=object()),'cleanup','finish')
            scope['finish']()
            self.assertEqual({call.args[0] for call in table.Delete.call_args_list},set([1,2]))
            self.assertNotIn('error',json.loads(Path(root,'response.json').read_text()))

    def test_optional_logging_failure_does_not_escape(self):
        scope=worker_functions(dict(root='unused',os=os,open=Mock(side_effect=OSError('log unavailable'))),'log')
        scope['log']('worker completed')

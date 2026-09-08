"""Validation and cleanup contracts for the isolated undo-selection worker."""
import ast
import copy
import json
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock
from .client import OracleProtocolError
from .undo_selection import validate_request

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

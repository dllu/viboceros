"""Exercise real worker dispatch without loading Rhino or its command helpers."""
import ast
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from . import structural_sdk


class StructuralDispatchTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = Path(__file__).with_name('rhino_worker.py')
        tree = ast.parse(path.read_text())
        function = next(node for node in tree.body
                        if isinstance(node, ast.FunctionDef) and node.name == '_execute')
        cls.worker_code = compile(ast.Module(body=[function], type_ignores=[]), str(path), 'exec')

    def worker(self):
        host = {'_sweep1': Mock(return_value=('inline', 7))}
        exec(self.worker_code, host)
        return host

    def test_worker_preserves_requests_context_timing_and_context_only_adapters(self):
        for kind, timed in [('sweep_refit_sections', True), ('grip_navigation', False)]:
            with self.subTest(kind=kind):
                host = self.worker()
                operation, tolerance = {'op': kind, 'id': 'owned'}, {'absolute': 1e-7}
                result = (object(), 123)
                callback = Mock(return_value=result)
                module = structural_sdk.PROBES[kind][0]
                with patch.dict(sys.modules, {'structural_sdk': structural_sdk,
                                              module: SimpleNamespace(run=callback)}):
                    self.assertIs(host['_execute'](operation, 3, tolerance), result)
                expected = (operation, host, 3, tolerance) if timed else (operation, host)
                callback.assert_called_once_with(*expected)

    def test_new_registration_reaches_worker_without_a_second_dispatch_declaration(self):
        host = self.worker()
        operation = {'op': 'registered_test_probe', 'id': 'owned'}
        callback = Mock(return_value=('registered', 19))
        with patch.dict(structural_sdk.PROBES, {'registered_test_probe': ('owned_test_probe', ())}), \
                patch.dict(sys.modules, {'structural_sdk': structural_sdk,
                                        'owned_test_probe': SimpleNamespace(run=callback)}):
            self.assertEqual(host['_execute'](operation, 2, {}), ('registered', 19))
        callback.assert_called_once_with(operation, host, 2, {})

    def test_inline_sweep_adapter_remains_reachable(self):
        host = self.worker()
        operation, tolerance = {'op': 'sweep1'}, {'absolute': 1e-7}
        with patch.dict(sys.modules, {'structural_sdk': structural_sdk}):
            self.assertEqual(host['_execute'](operation, 1, tolerance), ('inline', 7))
        host['_sweep1'].assert_called_once_with(operation, 1, tolerance)

    def test_unknown_operations_cannot_import_arbitrary_modules(self):
        for kind in ['unregistered_probe', 'os', 'sweep1']:
            with self.subTest(kind=kind):
                self.assertFalse(structural_sdk.can_dispatch(kind))
                with patch('builtins.__import__') as imported:
                    with self.assertRaisesRegex(ValueError, 'structural dispatch'):
                        structural_sdk.dispatch({'op': kind}, {}, 1, {})
                imported.assert_not_called()

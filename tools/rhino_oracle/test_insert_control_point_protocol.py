"""Original geometry operations exercise command picks and completed cancels."""
import copy
import gzip
import hashlib
import json
import tempfile
from pathlib import Path
from unittest import TestCase
from unittest.mock import patch
from types import SimpleNamespace

from .generate_insert_control_point_protocol import request
from .insert_control_point_geometry_probe import command_geometry, validate
from . import join_probe
from .structural_sdk import stage_helpers

HERE = Path(__file__).parent


class InsertControlPointGeometryProtocol(TestCase):
    def test_capture_has_exact_request_and_producer_hashes(self):
        q = request()
        fixture = HERE / 'fixtures/insert_control_point_protocol.json'
        self.assertEqual(q, json.loads(fixture.read_text()))
        captured = json.loads((HERE / 'observations/insert_control_point_protocol.json').read_text())
        self.assertEqual(len(captured['results']), 44)
        self.assertEqual([r['id'] for r in captured['results']], [o['id'] for o in q['operations']])
        self.assertEqual(hashlib.sha256(fixture.read_bytes()).hexdigest(), captured['provenance']['request_sha256'])
        for path, digest in captured['provenance']['inputs'].items():
            self.assertEqual(hashlib.sha256((HERE.parent.parent / path).read_bytes()).hexdigest(), digest, path)
        for name, digest in captured['provenance']['sources'].items():
            data = (HERE / name).read_bytes()
            if hashlib.sha256(data).hexdigest() != digest and name in ('rhino_worker.py', 'structural_sdk.py'):
                data = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)

    def test_original_nine_cases_are_retained(self):
        old = json.loads((HERE / 'fixtures/insert_control_point.json').read_text())
        self.assertEqual(request()['operations'][:9], old['operations'])

    def test_staging_includes_idle_dispatch_and_command_observer(self):
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(request(), HERE, Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()}, {
                'insert_control_point_geometry_probe.py', 'join_probe.py', 'merge_edges_probe.py'})
            for p in Path(d).iterdir():
                self.assertEqual(p.read_bytes(), (HERE / p.name).read_bytes())

    def test_malformed_inputs_are_rejected_before_native_staging(self):
        curve, surface = request()['operations'][0], request()['operations'][-1]
        for op in (dict(curve, midpoint=1), dict(curve, curve=[]),
                   dict(surface, direction='both'), dict(surface, parameter=[.5]),
                   {k: v for k, v in curve.items() if k != 'parameter'}):
            with self.subTest(op=op), self.assertRaises(ValueError):
                validate(copy.deepcopy(op))

    def test_cancel_requires_exactly_one_completed_native_command(self):
        class Event:
            def __init__(self): self.handlers = []
            def __iadd__(self, handler): self.handlers.append(handler); return self
            def __isub__(self, handler): self.handlers.remove(handler); return self
            def emit(self, name, result):
                for handler in self.handlers:
                    handler(None, SimpleNamespace(CommandEnglishName=name, CommandResult=result))

        source = SimpleNamespace(EndCommand=Event())
        with patch.dict('sys.modules', join_probe=join_probe):
            for result in ('Success', 'Cancel'):
                value = object()
                self.assertIs(command_geometry(source,
                    lambda: source.EndCommand.emit('InsertControlPoint', result), lambda: value), value)
                self.assertEqual(source.EndCommand.handlers, [])
            for events in ([], [('InsertControlPoint', 'Failure')], [('OtherCommand', 'Success')],
                           [('InsertControlPoint', 'Cancel')] * 2):
                def run():
                    for name, result in events: source.EndCommand.emit(name, result)
                with self.subTest(events=events), self.assertRaises(ValueError):
                    command_geometry(source, run, lambda: object())
                self.assertEqual(source.EndCommand.handlers, [])

            def failed_snapshot(): raise ValueError('cannot inspect native geometry')
            with self.assertRaisesRegex(ValueError, 'cannot inspect native geometry'):
                command_geometry(source, lambda: source.EndCommand.emit('InsertControlPoint', 'Cancel'), failed_snapshot)
            self.assertEqual(source.EndCommand.handlers, [])

"""Validation must reject malformed fixtures before accessing the Rhino host."""
import unittest
from types import SimpleNamespace
from unittest.mock import Mock
from . import join_probe


class JoinProbeTests(unittest.TestCase):
    def test_brep_commands_require_shared_artifacts_before_accessing_host(self):
        for brep in (None, [], {}, {"artifact_path": None}, {"artifact_path": 1}, {"artifact_path": ""}):
            with self.subTest(brep=brep), self.assertRaisesRegex(ValueError, "shared source artifacts"):
                join_probe.run({"sources": [{"brep": brep}]}, None, {})
        sources = [{"brep": {"artifact_path": "/owned/input.3dm"}}]
        self.assertEqual(join_probe.validate({"sources": sources}), (sources, [0]))

    def test_invalid_selections_never_touch_host(self):
        for selected in ([], [0, 0], [1], [-1], [True], [0.0], "0"):
            with self.subTest(selected=selected), self.assertRaisesRegex(ValueError, "selection"):
                join_probe.run({"sources": [{}], "selected": selected}, None, {})

    def test_invalid_options_and_source_counts_never_touch_host(self):
        for sources in ([], [{}] * 33, {}):
            with self.subTest(sources=sources), self.assertRaisesRegex(ValueError, "sources"):
                join_probe.run({"sources": sources}, None, {})
        for key in ("join_disjoint", "preselect", "trace_commands", "definition_only"):
            for value in (0, 1, "Yes", None):
                with self.subTest(key=key, value=value), self.assertRaisesRegex(ValueError, "boolean"):
                    join_probe.run({"sources": [{}], key: value}, None, {})
        for value in (0, -1, float("nan"), float("inf"), True, "0.1"):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "tolerance"):
                join_probe.run({"sources": [{}], "absolute_tolerance": value}, None, {})

    def test_valid_order_and_defaults_are_preserved(self):
        sources = [{}, {}, {}]
        self.assertEqual(join_probe.validate({"sources": sources}), (sources, [0, 1, 2]))
        self.assertEqual(join_probe.validate({"sources": sources, "selected": [2, 0],
            "join_disjoint": True, "preselect": False, "absolute_tolerance": 0.125}), (sources, [2, 0]))

    def test_command_cannot_inject_arbitrary_macros(self):
        for command in (None, True, "join", "Join _Delete", "Delete"):
            with self.subTest(command=command), self.assertRaisesRegex(ValueError, "command"):
                join_probe.run({"sources": [{}], "command": command}, None, {})
        for command in ("Join", "JoinCopy"):
            self.assertEqual(join_probe.validate({"sources": [{}], "command": command}), ([{}], [0]))

    def test_definition_only_records_never_request_samples_or_mass_properties(self):
        record = Mock(return_value={"complete": "definition"})
        source = SimpleNamespace(SolidOrientation="Inward", IsSolid=True,
            Edges=[SimpleNamespace(Valence="Interior")])
        host = {"Rhino": SimpleNamespace(Geometry=SimpleNamespace(
            EdgeAdjacency=SimpleNamespace(Interior="Interior"))),
            "_interchange_brep_record": record}
        actual = join_probe.record_brep(source, None, host, definition_only=True)
        self.assertEqual(actual, {"orientation": "Inward", "solid": True,
            "closed": True, "geometry": {"complete": "definition"}})
        record.assert_called_once_with(source, include_samples=False)


class Event:
    def __init__(self): self.handlers = []
    def __iadd__(self, handler):
        self.handlers.append(handler)
        return self
    def __isub__(self, handler):
        self.handlers.remove(handler)
        return self
    def fire(self, name, result="Success"):
        for handler in self.handlers:
            handler(None, SimpleNamespace(CommandEnglishName=name, CommandResult=result))


class JoinCommandObservationTests(unittest.TestCase):
    def test_result_and_snapshot_belong_to_the_named_command_not_macro_tail(self):
        source = SimpleNamespace(EndCommand=Event())
        state = [1]
        def run():
            source.EndCommand.fire("JoinCopy", "Cancel")
            state[:] = [2]
            source.EndCommand.fire("SelID")
            return True
        succeeded, objects, trace = join_probe.observe_command(source, "JoinCopy", run,
            lambda: list(state), lambda: list(state), True)
        self.assertFalse(succeeded)
        self.assertEqual(objects, [1])
        self.assertEqual([e["selected"] for e in trace], [[1], [2]])
        self.assertEqual(source.EndCommand.handlers, [])

    def test_detaches_after_runner_failure_and_rejects_missing_or_duplicate_completions(self):
        source = SimpleNamespace(EndCommand=Event())
        def fail(): raise RuntimeError("script failed")
        with self.assertRaisesRegex(RuntimeError, "script failed"):
            join_probe.observe_command(source, "Join", fail, list, list)
        self.assertEqual(source.EndCommand.handlers, [])
        for count in (0,2):
            with self.assertRaisesRegex(ValueError, "exactly one"):
                join_probe.observe_command(source, "Join",
                    lambda: [source.EndCommand.fire("Join") for _ in range(count)], list, list)
            self.assertEqual(source.EndCommand.handlers, [])

    def test_swallowed_event_handler_error_cannot_publish_a_partial_snapshot(self):
        source = SimpleNamespace(EndCommand=Event())
        def fail(): raise RuntimeError("snapshot failed")
        with self.assertRaisesRegex(ValueError, "snapshot failed"):
            join_probe.observe_command(source, "Join", lambda: source.EndCommand.fire("Join"), fail, list)
        self.assertEqual(source.EndCommand.handlers, [])

"""Tests for the host-side Rhino oracle API."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from .client import (
    OracleError,
    OracleProtocolError,
    _run_logged,
    compare_responses,
    load_request,
    posix_to_wine_path,
)


def _response(engine: str, value: object, elapsed_ns: int = 100) -> dict:
    return {
        "protocol_version": 1,
        "engine": engine,
        "engine_version": "test",
        "iterations": 10,
        "results": [
            {"id": "operation", "value": value, "elapsed_ns": elapsed_ns}
        ],
    }


class OracleClientTests(unittest.TestCase):
    def test_compares_nested_values_and_reports_timings(self) -> None:
        viboceros = _response(
            "viboceros", {"point": [1.0, 2.0, 3.0], "radius": 4.0}, 100
        )
        rhino = _response(
            "rhino", {"point": [1.0, 2.0 + 1.0e-12, 3.0], "radius": 4.0}, 400
        )
        report = compare_responses(viboceros, rhino, 1.0e-10, 1.0e-10)
        self.assertTrue(report.passed)
        self.assertAlmostEqual(report.max_absolute_error, 1.0e-12, places=15)
        self.assertEqual(report.operations[0].viboceros_ns_per_iteration, 10.0)
        self.assertEqual(report.operations[0].rhino_to_viboceros_ratio, 4.0)

    def test_reports_out_of_epsilon_and_structural_differences(self) -> None:
        viboceros = _response("viboceros", {"point": [1.0, 2.0], "flag": True})
        rhino = _response("rhino", {"point": [1.0, 2.1], "other": True})
        report = compare_responses(viboceros, rhino, 1.0e-6, 1.0e-6)
        self.assertFalse(report.passed)
        self.assertGreater(report.max_absolute_error, 0.09)
        self.assertEqual(len(report.operations[0].differences), 2)

    def test_rejects_engine_errors_and_mismatched_ids(self) -> None:
        failed = _response("rhino", 1.0)
        failed["error"] = "bad geometry"
        with self.assertRaisesRegex(OracleError, "bad geometry"):
            compare_responses(_response("viboceros", 1.0), failed)

        other = _response("rhino", 1.0)
        other["results"][0]["id"] = "other"
        with self.assertRaisesRegex(OracleProtocolError, "result ids"):
            compare_responses(_response("viboceros", 1.0), other)

        non_finite = _response("rhino", float("nan"))
        with self.assertRaisesRegex(OracleProtocolError, "non-finite"):
            compare_responses(_response("viboceros", 1.0), non_finite)

    def test_maps_absolute_paths_to_wines_z_drive(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            mapped = posix_to_wine_path(Path(directory) / "worker.py")
        self.assertTrue(mapped.startswith("Z:\\"))
        self.assertTrue(mapped.endswith("\\worker.py"))
        self.assertNotIn("/", mapped)

    def test_load_request_requires_an_object(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            request = Path(directory) / "request.json"
            request.write_text("[]", encoding="utf-8")
            with self.assertRaisesRegex(OracleProtocolError, "root"):
                load_request(request)

    def test_logged_launcher_capture_preserves_diagnostics(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            completed = _run_logged(
                ["sh", "-c", "printf standard; printf error >&2"],
                root,
                5.0,
                root / "launcher.log",
            )
        self.assertEqual(completed.returncode, 0)
        self.assertEqual(completed.stdout, "standarderror")
        self.assertEqual(completed.stderr, "")


if __name__ == "__main__":
    unittest.main()

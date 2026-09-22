"""Independent exact box witnesses; raw Rhino and native-only evidence stay separate."""
import hashlib
import json
import math
from fractions import Fraction as F
from pathlib import Path
import unittest

from .references.join_orientation import request, repeat_request

ROOT = Path(__file__).resolve().parents[2]
ORACLE = ROOT / "tools/rhino_oracle"


def load(name):
    return json.loads((ORACLE / name).read_text())


def face_senses(geometry, scale):
    center = [scale / 2, scale, 2 * scale]
    for surface, topology in zip(geometry["faces"], geometry["topology"]["faces"]):
        definition = surface["definition"]
        assert definition["degree"] == [1, 1]
        assert definition["control_count"] == [2, 2]
        assert all(cp["weight"] == 1 for cp in definition["control_points"])
        p = [[F(x) for x in cp["point"]] for cp in definition["control_points"]]
        assert p[3] == [p[1][i] + p[2][i] - p[0][i] for i in range(3)]
        u = [p[1][i] - p[0][i] for i in range(3)]
        v = [p[2][i] - p[0][i] for i in range(3)]
        normal = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]]
        face_center = [sum(point[i] for point in p) / 4 for i in range(3)]
        dot = sum(normal[i] * (face_center[i] - center[i]) for i in range(3))
        if topology["reversed"]:
            dot = -dot
        assert dot != 0
        yield dot > 0


class JoinOrientationTests(unittest.TestCase):
    def test_source_only_matrices_and_retained_capture_hashes(self):
        for name, exponent in [("join_orientation", 0), ("join_orientation_tiny", -360),
                               ("join_orientation_large", 360)]:
            fixture = load("fixtures/" + name + ".json")
            self.assertEqual(fixture, request(exponent))
            self.assertEqual(len(fixture["operations"]), 32)
            self.assertEqual(len({op["id"] for op in fixture["operations"]}), 32)
        self.assertEqual(load("fixtures/join_orientation_large_repeat.json"), repeat_request())
        provenance = json.loads((ROOT / "docs/join-orientation-provenance.json").read_text())
        for path, expected in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT / path).read_bytes()).hexdigest(), expected)

    def test_exact_normals_expose_large_rhino_getter_and_normalization_discrepancies(self):
        closed, opened, inward_closed = 0, 0, 0
        for name, exponent in [("join_orientation", 0), ("join_orientation_large", 360)]:
            observed = load("observations/" + name + ".json")
            self.assertEqual(len(observed["results"]), 32)
            scale = F(math.ldexp(1.0, exponent))
            for op, row in zip(request(exponent)["operations"], observed["results"]):
                with self.subTest(id=op["id"]):
                    self.assertEqual(row["id"], op["id"])
                    self.assertTrue(row["value"]["succeeded"])
                    outputs = [obj for obj in row["value"]["objects"] if obj["source"] is None]
                    self.assertEqual(len(outputs), 1)
                    record = outputs[0]["brep"]
                    self.assertEqual(record["solid"], op["preselect"])
                    self.assertEqual(record["orientation"], "Outward" if op["preselect"] and exponent==0 else "None")
                    expected_sense = (op["preselect"] and exponent==0) or not op["sources"][0]["brep"]["reversed"]
                    senses = list(face_senses(record["geometry"], scale))
                    self.assertEqual(senses, [expected_sense] * (6 if op["preselect"] else 5))
                    closed += op["preselect"]
                    opened += not op["preselect"]
                    inward_closed += op["preselect"] and not expected_sense
        self.assertEqual((closed, opened), (32, 32))
        self.assertEqual(inward_closed, 8)

    def test_native_underflow_counterexamples_are_not_rhino_observations(self):
        before = load("observations/join_orientation_tiny_native_before.json")
        self.assertEqual(before["engine"], "viboceros")
        self.assertEqual(len(before["outcomes"]), 32)
        inward = 0
        scale = F(math.ldexp(1.0, -360))
        self.assertEqual(8 * scale**3, F(1, 2**1077))
        self.assertEqual(float(8 * scale**3), 0.0)
        self.assertGreater(8 * F(2**360)**3, F(float.fromhex("0x1.fffffffffffffp+1023")))
        for outcome in before["outcomes"]:
            self.assertEqual(outcome["status"], "success")
            outputs = [obj["brep"] for obj in outcome["result"]["value"]["objects"] if obj["source"] is None]
            self.assertEqual(len(outputs), 1)
            record = outputs[0]
            if record["solid"]:
                outside = record["orientation"] == "Outward"
                self.assertEqual(list(face_senses(record["geometry"], scale)), [outside] * 6)
                inward += not outside
        self.assertEqual(inward, 8)

    def test_fresh_repeat_preserves_whole_records_and_isolates_uninserted_getter(self):
        observed = load("observations/join_orientation_large.json")
        original = {row["id"]: row for row in observed["results"]}
        repeat = load("observations/join_orientation_large_repeat.json")
        self.assertEqual(len(repeat["results"]), 10)
        for row in repeat["results"][:8]:
            self.assertEqual(row, original[row["id"]])
        for row, inward in zip(repeat["results"][8:], (False, True)):
            self.assertEqual(row["id"], "uninserted-large-reversed-%s" % inward)
            record = row["value"]
            self.assertTrue(record["solid"])
            self.assertTrue(record["closed"])
            self.assertEqual(record["orientation"], "None")
            self.assertEqual(list(face_senses(record["geometry"], F(2**360))), [not inward] * 6)

    def test_initial_uncapturable_source_attempt_is_retained_as_failures(self):
        discovery = load("observations/join_orientation_source_discovery.json")
        self.assertEqual(discovery["request"]["tolerance"]["absolute"], math.ldexp(1.0, -400))
        self.assertEqual(len(discovery["native_audit"]["outcomes"]), 96)
        tiny = 0
        for outcome in discovery["native_audit"]["outcomes"]:
            self.assertEqual(outcome["status"], "failure")
            if outcome["id"].startswith("scale--360-"):
                self.assertEqual(outcome["error"]["kind"], "interchange")
                self.assertIn("closed surface", outcome["error"]["message"])
                tiny += 1
            else:
                self.assertEqual(outcome["error"]["kind"], "geometry")
                self.assertIn("lifted p-curve", outcome["error"]["message"])
        self.assertEqual(tiny, 32)

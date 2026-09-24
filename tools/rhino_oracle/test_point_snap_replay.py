"""Calibrated replay inputs, failure reporting and retained competition evidence."""
import copy
import hashlib
import json
import math
import unittest
from pathlib import Path
from unittest.mock import Mock
from . import point_snap_replay as replay
from .client import OracleError, OracleProtocolError
from .references import mesh_near, mesh_snap_order

ROOT = Path(__file__).resolve().parents[2]


def inputs(name="point_snaps"):
    return [json.loads((ROOT/("tools/rhino_oracle/"+folder+"/"+name+".json")).read_text())
            for folder in ("fixtures","observations")]


class PointSnapReplayTests(unittest.TestCase):
    def test_multi_object_intersection_captures_prepare_for_native_replay(self):
        for stem in ("intersection_multi_snaps", "intersection_multi_detail_snaps",
                     "intersection_multi_depth_snaps", "intersection_multi_orientation_snaps",
                     "intersection_multi_motion_snaps", "intersection_vertical_sweep_snaps",
                     "intersection_rotated_priority_snaps", "intersection_near_vertical_coarse_snaps",
                     "intersection_near_vertical_tiny_snaps"):
            with self.subTest(stem=stem):
                request,observed = inputs(stem)
                native,evidence = replay.prepare(request,observed)
                self.assertEqual(len(native["operations"]),len(request["operations"]))
                self.assertTrue(all(row["value"]["kind"] == "Intersection" for row in evidence["results"]))
                self.assertTrue(all(op["op"] == "projected_object_snap" for op in native["operations"]))

    def test_targets_never_enter_native_input_and_misses_do_not_assert_placement(self):
        request,observed = inputs()
        native,evidence = replay.prepare(request,observed)
        self.assertEqual(len(native["operations"]),101)
        self.assertEqual(sum(row["value"]["point"] is None for row in evidence["results"]),8)
        for row in observed["results"]: row["value"]["point"] = [123.,456.,789.]
        other,changed = replay.prepare(request,observed)
        self.assertEqual(other,native)
        self.assertNotEqual(changed,evidence)
        for op in native["operations"]:
            self.assertEqual(set(op),{"op","id","sources","camera","cursor","capture_radius","modes","snap_to_meshes"})

    def test_invalid_observations_fail_before_native_execution(self):
        request,observed = inputs()
        mutations = [
            lambda r:r["results"].pop(),
            lambda r:r["results"].reverse(),
            lambda r:r.update(iterations=2),
            lambda r:r["results"][0]["value"].update(source=True),
            lambda r:r["results"][0]["value"].update(kind="Unknown"),
            lambda r:r["results"][0]["value"].update(point=None),
            lambda r:r["results"][0]["value"].update(before=[]),
            lambda r:r["results"][0]["value"].update(after=[]),
            lambda r:r["results"][0]["value"]["mesh_snap_setting"].update(restored=None),
            lambda r:r["results"][0]["value"]["frame"].update(camera_direction=[0,0,0]),
            lambda r:r["results"][0]["value"]["frame"].update(aim_client=[123,456]),
            lambda r:r["results"][0]["value"]["frame"].update(click_client=[0,0]),
            lambda r:r["results"][0]["value"]["frame"].update(world_to_screen=[[0,0,0,0]]*4),
        ]
        client = Mock()
        for mutation in mutations:
            corrupted = copy.deepcopy(observed); mutation(corrupted)
            with self.assertRaises(OracleProtocolError): replay.replay(request,corrupted,client)
        client.run_viboceros.assert_not_called()
        for epsilon in (True,-1,float("nan")):
            with self.assertRaises(OracleProtocolError): replay.replay(request,observed,client,epsilon)
        client.run_viboceros.assert_not_called()

    def test_mismatches_and_engine_errors_are_not_turned_into_passes(self):
        request,observed = inputs()
        native,evidence = replay.prepare(request,observed)
        actual = dict(copy.deepcopy(evidence),engine="viboceros")
        client = Mock(); client.run_viboceros.return_value = actual
        self.assertTrue(replay.replay(request,observed,client).passed)
        client.run_viboceros.assert_called_once_with(native)
        actual["results"][0]["value"]["point"][0] += 1.
        report = replay.replay(request,observed,client)
        self.assertFalse(report.passed)
        self.assertEqual(sum(not row.passed for row in report.operations),1)
        self.assertGreater(report.max_absolute_error,0.9)
        client.run_viboceros.side_effect = OracleError("native failed")
        with self.assertRaises(OracleError): replay.replay(request,observed,client)

    def test_competition_evidence_preserves_public_wire_order_and_repeatability(self):
        request,observed = inputs("mesh_snap_order")
        self.assertEqual(request,mesh_snap_order.all_request())
        replay.prepare(request,observed)
        self.assertEqual(len(observed["results"]),84)
        discoveries = {}
        for op,row in zip(request["operations"],observed["results"]):
            value = row["value"]; frame = value["frame"]
            self.assertEqual(value["kind"],"Near")
            self.assertEqual(value["source"],0)
            self.assertEqual(value["component"]["type"],"MeshTopologyEdge")
            mesh = op["sources"][0]
            edges = sorted(set(tuple(sorted((face[i],face[(i+1)%len(face)])))
                               for face in mesh["faces"] for i in range(len(face))))
            wires = [[mesh["vertices"][i] for i in edge] for edge in edges]
            self.assertEqual(value["topology_wires"],[wires])
            # This verifies the target ON THE OBSERVED WIRE, not wire selection.
            a,b = wires[value["component"]["index"]]
            target,_ = mesh_near.closest([frame["world_to_screen"][i] for i in (0,1,3)],a,b,frame["click_client"],12)
            self.assertLess(math.dist(value["point"],list(map(float,target))),1e-9)
            if op["id"].startswith("repeat-"):
                original = discoveries[op["id"].split("-",2)[2]]
                self.assertEqual(value["component"],original["component"])
                self.assertLess(math.dist(value["point"],original["point"]),1e-9)
            elif not op["id"].startswith("picking-"):
                discoveries[op["id"]] = value
        self.assertNotEqual(discoveries["order-0-0--10"]["point"],discoveries["order-0-1--10"]["point"])

    def test_picking_is_not_a_substitute_for_snap_selection(self):
        _,observed = inputs("mesh_snap_order")
        rows = {row["id"]:row["value"] for row in observed["results"]}
        for name in ("picking-order-0-0--10","picking-parallel-top-10","picking-parallel-perspective--4"):
            value = rows[name]; picks = value["wire_picks"]
            selected = value["component"]["index"]
            ordinary = picks["meshes"][0]
            self.assertEqual(ordinary["flag"],"Edge")
            self.assertNotEqual(selected,ordinary["index"])
            self.assertGreater(math.dist(value["point"],ordinary["point"]),0.4)
        # The first corner's actual Near wire has both WORSE public line-pick
        # depth (larger is nearer) and distance than the other admitted wire.
        value = rows["picking-order-0-0--10"]
        lines = value["wire_picks"]["sources"][0]
        self.assertLess(lines[1]["depth"],lines[0]["depth"])
        self.assertGreater(lines[1]["distance"],lines[0]["distance"])
        for value in (rows[op["id"]] for op in mesh_snap_order.picking_request()["operations"]):
            matrix = value["wire_picks"]["transform"]
            for ends,pick in zip(value["topology_wires"][0],value["wire_picks"]["sources"][0]):
                if pick is None: continue
                self.assertTrue(0 <= pick["t"] <= 1)
                self.assertTrue(all(math.isfinite(pick[key]) for key in ("t","depth","distance")))
                self.assertEqual(len(matrix),4)

    def test_retained_competition_provenance_hashes(self):
        provenance = json.loads((ROOT/"docs/mesh-snap-order-provenance.json").read_text())
        for path,digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest)


if __name__ == "__main__": unittest.main()

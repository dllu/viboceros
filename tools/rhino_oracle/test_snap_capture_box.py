"""Retained square-admission evidence, including unsuccessful selection parity."""
import hashlib
import json
import math
import unittest
from . import point_snap_replay
from .references import mesh_near, mesh_snap_sources, projected_lines, projected_near, snap_capture_box
from .references.point_snaps import reference_target
from .test_point_snap_replay import ROOT, inputs


BOX_TARGET_DIFFERENCES = {
    "box-top-mesh-near--10-10", "box-perspective-mesh-near--8--8",
    "box-perspective-mesh-near--10--10", "box-perspective-mesh-near--10-10",
    "box-perspective-mesh-near-10--10",
}


def independent_target(operation, frame):
    candidates = []
    for index,source in enumerate(operation["sources"]):
        target = reference_target(dict(operation,sources=[source]),frame)
        if target: candidates.append(dict(target,source=index))
    # All multiple-source fixtures are Near-only, so there is no kind priority
    # or Mid-hover score to infer from the target point.
    return min(candidates,key=lambda p:projected_near.distance(frame,p["point"])) if candidates else None


class SnapCaptureBoxTests(unittest.TestCase):
    def test_full_corpora_regenerate_and_prepare_without_observed_target_inputs(self):
        for name,factory,count in (("snap_capture_box",snap_capture_box.all_request,128),
                                   ("mesh_snap_sources",mesh_snap_sources.request,72)):
            request,observed = inputs(name)
            self.assertEqual(request,factory())
            native,evidence = point_snap_replay.prepare(request,observed)
            self.assertEqual(len(native["operations"]),count)
            self.assertEqual(len(evidence["results"]),count)
            for row in observed["results"]: row["value"]["point"] = [17.,29.,31.]
            self.assertEqual(point_snap_replay.prepare(request,observed)[0],native)

    def test_square_admission_preserves_all_58_misses_and_five_selection_differences(self):
        request,observed = inputs("snap_capture_box")
        mismatches,misses,corners,hover = set(),0,0,0
        for op,row in zip(request["operations"],observed["results"]):
            value = row["value"]; frame = value["frame"]; radius = op.get("capture_radius",12)
            target = independent_target(op,frame)
            with self.subTest(id=op["id"]):
                if value["kind"] == "None":
                    self.assertIsNone(target)
                    misses += 1
                    continue
                self.assertIsNotNone(target)
                self.assertEqual((value["kind"],value["source"]),(target["kind"],target["source"]))
                if math.dist(value["point"],target["point"]) >= 1e-9: mismatches.add(op["id"])
                if "line-mid" in op["id"]:
                    self.assertGreater(projected_near.distance(frame,value["point"]),3*radius)
                    source = op["sources"][0]
                    locus = projected_near.line_point(frame,source["start"],source["end"])
                    self.assertTrue(projected_near.in_square(frame,locus,radius))
                    hover += 1
                else:
                    self.assertTrue(projected_near.in_square(frame,value["point"],radius))
                    if projected_near.distance(frame,value["point"]) > radius: corners += 1
        self.assertEqual((misses,corners,hover),(58,39,10))
        self.assertEqual(mismatches,BOX_TARGET_DIFFERENCES)

    def test_slanted_curves_do_not_clip_an_outside_closest_point_into_the_box(self):
        request,observed = inputs("snap_capture_box")
        rejected_crossings,corner_hits = 0,0
        for op,row in zip(request["operations"],observed["results"]):
            if not op["id"].startswith("slanted-0-") or op["capture_radius"] != 12: continue
            value = row["value"]; frame = value["frame"]; source = op["sources"][0]
            a,b = source["start"],source["end"]
            matrix = [frame["world_to_screen"][i] for i in (0,1,3)]
            nearest,_ = projected_lines.closest(matrix,a,b,frame["click_client"],1.)
            if value["kind"] == "None":
                self.assertFalse(projected_near.in_square(frame,nearest,12))
                # A sampled point actually lies inside, not just the wire's bbox.
                self.assertTrue(any(projected_near.in_square(frame,[(1-t/100)*x+t/100*y for x,y in zip(a,b)],12)
                                    for t in range(101)))
                rejected_crossings += 1
            else:
                self.assertGreater(projected_near.distance(frame,value["point"]),12)
                self.assertLess(math.dist(value["point"],list(map(float,nearest))),1e-9)
                corner_hits += 1
        self.assertEqual((rejected_crossings,corner_hits),(6,2))

    def test_separate_sources_match_and_differences_are_within_combined_meshes(self):
        request,observed = inputs("mesh_snap_sources")
        separate,matches,differences = 0,0,0
        for op,row in zip(request["operations"],observed["results"]):
            value = row["value"]; target = independent_target(op,value["frame"])
            self.assertEqual(value["kind"],"Near")
            self.assertEqual(value["source"],target["source"])
            match = math.dist(value["point"],target["point"]) < 1e-9
            if "combined" not in op["id"]:
                self.assertTrue(match,op["id"])
                separate += 1
            matches += match
            differences += not match
        self.assertEqual((separate,matches,differences),(48,65,7))

    def test_per_wire_reference_includes_eight_short_wire_endpoint_corrections(self):
        checked,endpoint_controls = 0,set()
        for name in ("snap_capture_box","mesh_snap_sources"):
            request,observed = inputs(name)
            for op,row in zip(request["operations"],observed["results"]):
                value = row["value"]
                if value["kind"] != "Near" or op["sources"][value["source"]]["type"] != "mesh": continue
                frame = value["frame"]
                a,b = value["topology_wires"][value["source"]][value["component"]["index"]]
                matrix = [frame["world_to_screen"][i] for i in (0,1,3)]
                target,_ = mesh_near.closest(matrix,a,b,frame["click_client"],op.get("capture_radius",12))
                self.assertLess(math.dist(value["point"],list(map(float,target))),1e-9,op["id"])
                checked += 1
                if op["id"].startswith("slanted-1-") and op["capture_radius"] == 16:
                    # Both endpoints are inside: ordinary curve proximity must
                    # not replace the measured mesh endpoint behavior.
                    ordinary,_ = projected_lines.closest(matrix,a,b,frame["click_client"],1.)
                    self.assertIn(value["point"],(a,b))
                    self.assertNotIn(list(map(float,ordinary)),(a,b))
                    self.assertGreater(math.dist(value["point"],list(map(float,ordinary))),0.2)
                    endpoint_controls.add(op["id"])
        self.assertEqual(checked,68)
        self.assertEqual(endpoint_controls,{"slanted-1-%d-%d-r16" % (r,reverse) for r in range(4) for reverse in range(2)})

    def test_retained_provenance_hashes(self):
        provenance = json.loads((ROOT/"docs/snap-capture-box-provenance.json").read_text())
        for path,digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest)


if __name__ == "__main__": unittest.main()

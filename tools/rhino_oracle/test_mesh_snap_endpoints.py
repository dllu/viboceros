"""Endpoint calculation, mesh selection, and host timing are separate evidence."""
import hashlib
import json
import math
import unittest
from . import point_snap_replay
from .references import mesh_near, mesh_snap_endpoints, projected_near
from .test_point_snap_replay import ROOT, inputs
from .test_snap_capture_box import independent_target


CORPORA = (("mesh_snap_endpoints",mesh_snap_endpoints.request,80,59),
           ("mesh_snap_endpoint_settling",mesh_snap_endpoints.settled_request,120,93),
           ("mesh_snap_edge_on",mesh_snap_endpoints.edge_on_request,72,58))


class MeshSnapEndpointTests(unittest.TestCase):
    def test_generators_regenerate_every_case_and_unchanged_sources_match_public_topology(self):
        for name,factory,count,_ in CORPORA:
            request,observed=inputs(name)
            self.assertEqual(request,factory())
            native,_=point_snap_replay.prepare(request,observed)
            self.assertEqual(len(native["operations"]),count)
            for op,row in zip(request["operations"],observed["results"]):
                for source,wires in zip(op["sources"],row["value"]["topology_wires"]):
                    if source["type"]=="mesh": self.assertEqual(mesh_near.wires(source),wires)

    def test_all_timing_repetitions_preserve_targets_components_cameras_and_public_picks(self):
        _,original=inputs("mesh_snap_endpoints")
        baseline={row["id"]:row["value"] for row in original["results"]}
        request,observed=inputs("mesh_snap_endpoint_settling")
        point_snap_replay.prepare(request,observed)
        delays={0:0,250:0,750:0}
        for op,row in zip(request["operations"],observed["results"]):
            _,delay,name=row["id"].split("-",2); delay=int(delay)
            value=dict(row["value"])
            if delay:
                motion=value.pop("input_motion")
                self.assertEqual(motion["requested_settle_ms"],delay)
                self.assertGreaterEqual(motion["motion_to_click_ms"],delay)
            self.assertEqual(value,baseline[name],op["id"])
            delays[delay]+=1
        self.assertEqual(delays,{0:40,250:40,750:40})

    def test_520_public_line_picks_match_independent_endpoint_and_interior_reference(self):
        tested,captured,ties=0,0,0
        for name in ("mesh_snap_order","mesh_snap_endpoints","mesh_snap_edge_on"):
            request,observed=inputs(name)
            for op,row in zip(request["operations"],observed["results"]):
                value=row["value"]; frame=value["frame"]; radius=op.get("capture_radius",12)
                if "wire_picks" not in value: continue
                matrix=[frame["world_to_screen"][i] for i in (0,1,3)]
                for wires,picks in zip(value["topology_wires"],value["wire_picks"]["sources"]):
                    if wires is None: continue
                    for (a,b),pick in zip(wires,picks):
                        target,_=mesh_near.closest(matrix,a,b,frame["click_client"],radius)
                        point=list(map(float,target))
                        self.assertEqual(projected_near.in_square(frame,point,radius),pick is not None,op["id"])
                        if pick is not None:
                            self.assertTrue(0<=pick["t"]<=1)
                            expected=[(1-pick["t"])*x+pick["t"]*y for x,y in zip(a,b)]
                            self.assertLess(math.dist(point,expected),1e-9,op["id"])
                            if all(projected_near.in_square(frame,p,radius) for p in (a,b)):
                                da,db=[projected_near.distance(frame,p) for p in (a,b)]
                                self.assertIn(point,(a,b))
                                if da==db:
                                    self.assertEqual(point,a)
                                    ties+=1
                            captured+=1
                        tested+=1
        self.assertEqual((tested,captured,ties),(520,300,36))

    def test_getpoint_per_wire_calculation_agrees_while_selection_differences_remain(self):
        captures,misses=0,0
        for name,_,_,expected_matches in CORPORA:
            request,observed=inputs(name); matches=0
            for op,row in zip(request["operations"],observed["results"]):
                value=row["value"]; frame=value["frame"]
                target=independent_target(op,frame)
                if value["kind"]=="None":
                    self.assertIsNone(target); misses+=1; matches+=1
                    continue
                self.assertIsNotNone(target)
                self.assertEqual((value["kind"],value["source"]),(target["kind"],target["source"]))
                matches+=math.dist(value["point"],target["point"])<1e-9
                if op["sources"][value["source"]]["type"]=="mesh":
                    a,b=value["topology_wires"][value["source"]][value["component"]["index"]]
                    matrix=[frame["world_to_screen"][i] for i in (0,1,3)]
                    point,_=mesh_near.closest(matrix,a,b,frame["click_client"],op["capture_radius"])
                    self.assertLess(math.dist(value["point"],list(map(float,point))),1e-9,op["id"])
                captures+=1
            self.assertEqual(matches,expected_matches,name)
        self.assertEqual((captures,misses),(196,76))

    def test_retained_provenance_hashes(self):
        provenance=json.loads((ROOT/"docs/mesh-snap-endpoints-provenance.json").read_text())
        for path,digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest)


if __name__=="__main__": unittest.main()

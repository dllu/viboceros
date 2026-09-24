"""Bounded unconstrained snap input and owned resource cleanup."""
import copy
import json
import math
import hashlib
import sys
import unittest
from types import SimpleNamespace
from unittest.mock import Mock, mock_open, patch
from contextlib import contextmanager
from pathlib import Path
from . import point_snap_probe as probe
from .references import mesh_near, projected_lines, projected_near
from .references.point_snaps import request, all_request, reference_target

ROOT = Path(__file__).resolve().parents[2]


class PointSnapTests(unittest.TestCase):
    def test_vertex_snap_is_separate_from_mesh_wire_switch(self):
        request = json.loads((ROOT/"tools/rhino_oracle/fixtures/mesh_vertex_snaps.json").read_text())
        observed = json.loads((ROOT/"tools/rhino_oracle/observations/mesh_vertex_snaps.json").read_text())
        probe.validate_request(request)
        self.assertEqual(len(request["operations"]), 6)
        self.assertEqual([op["id"] for op in request["operations"]],
                         [row["id"] for row in observed["results"]])
        for op, row in zip(request["operations"], observed["results"]):
            value = row["value"]
            self.assertEqual(value["before"], value["after"])
            self.assertEqual(value["mesh_snap_setting"]["before"],
                             value["mesh_snap_setting"]["restored"])
            if op["persistent_snaps"] == ["Vertex"]:
                self.assertEqual(value["kind"], "Vertex")
                self.assertEqual(value["point"], [0, 0, 0])
                self.assertEqual(value["source"], 0)
                self.assertEqual(value["component"], {"type": "MeshVertex", "index": 0})
            else:
                self.assertEqual(value["kind"], "None")
                self.assertIsNone(value["source"])
        mixed = json.loads((ROOT/"tools/rhino_oracle/fixtures/mesh_vertex_mixed_snaps.json").read_text())
        observations = json.loads((ROOT/"tools/rhino_oracle/observations/mesh_vertex_mixed_snaps.json").read_text())
        probe.validate_request(mixed)
        self.assertEqual([op["id"] for op in mixed["operations"]],
                         [row["id"] for row in observations["results"]])
        self.assertEqual(len(observations["results"]), 3)
        for row in observations["results"]:
            self.assertEqual(row["value"]["kind"], "Vertex")
            self.assertEqual(row["value"]["point"], [0, 0, 0])
        aperture = json.loads((ROOT/"tools/rhino_oracle/fixtures/mesh_vertex_aperture_snaps.json").read_text())
        edges = json.loads((ROOT/"tools/rhino_oracle/observations/mesh_vertex_aperture_snaps.json").read_text())
        probe.validate_request(aperture)
        self.assertEqual([op["id"] for op in aperture["operations"]],
                         [row["id"] for row in edges["results"]])
        self.assertEqual([row["value"]["kind"] for row in edges["results"]],
                         ["Vertex", "Vertex", "Near"])

    def test_full_3d_targets_and_three_corner_priority_differences_are_retained(self):
        operations = json.loads((ROOT/"tools/rhino_oracle/fixtures/point_snaps.json").read_text())
        self.assertEqual(operations,all_request())
        observed = json.loads((ROOT/"tools/rhino_oracle/observations/point_snaps.json").read_text())
        targets = json.loads((ROOT/"tools/rhino_oracle/fixtures/point_snap_targets.json").read_text())
        self.assertEqual(observed["engine_version"],"8.32.26160.13001")
        self.assertEqual(len(operations["operations"]),101)
        self.assertEqual(len(observed["results"]),101)
        probe.validate_request(operations)
        counts = dict(matches=0,misses=0,priority_differences=0)
        for op,row in zip(operations["operations"],observed["results"]):
            with self.subTest(id=op["id"]):
                self.assertEqual(op["id"],row["id"])
                value = row["value"]; frame = value["frame"]
                self.assertEqual(value["before"],value["after"])
                setting = value["mesh_snap_setting"]
                self.assertEqual(setting["before"],setting["restored"])
                self.assertEqual(setting["requested"],op["snap_to_meshes"])
                self.assertLess(math.dist(projected_near.project(frame,op["aim"]),frame["aim_client"]),1e-7)
                target = reference_target(op,frame)
                self.assertEqual(target,targets[op["id"]])
                if target is None:
                    self.assertEqual(value["kind"],"None")
                    self.assertIsNone(value["source"])
                    counts["misses"] += 1
                    continue
                self.assertEqual(value["kind"],target["kind"])
                self.assertEqual(value["source"],0)
                if op["id"] in ("threshold-tilted--10-0","threshold-tilted--9-0","aperture-tilted--8-0-r16"):
                    self.assertEqual(value["point"],op["sources"][0]["vertices"][0])
                    self.assertGreater(math.dist(value["point"],target["point"]),0.5)
                    self.assertLess(projected_near.distance(frame,target["point"]),projected_near.distance(frame,value["point"]))
                    if "component" in value:
                        self.assertEqual(value["component"],dict(type="MeshTopologyEdge",index=1))
                    counts["priority_differences"] += 1
                else:
                    self.assertLess(math.dist(value["point"],target["point"]),1e-9)
                    counts["matches"] += 1
        self.assertEqual(counts,dict(matches=90,misses=8,priority_differences=3))

    def test_aperture_branch_is_square_not_radial_and_matches_new_radius_witnesses(self):
        observed = json.loads((ROOT/"tools/rhino_oracle/observations/point_snaps.json").read_text())
        rows = {row["id"]:row["value"] for row in observed["results"]}
        value = rows["aperture-side--4-5-r12"]; frame = value["frame"]
        a,b = [7.,2.,-2.],[7.,8.,-2.]
        endpoint = projected_near.project(frame,a)
        delta = [x-y for x,y in zip(endpoint,frame["click_client"])]
        self.assertLess(max(map(abs,delta)),12)
        self.assertGreater(math.hypot(*delta),12)
        matrix = [frame["world_to_screen"][i] for i in (0,1,3)]
        ordinary,_ = projected_lines.closest(matrix,a,b,frame["click_client"],0.01)
        weighted,_ = mesh_near.closest(matrix,a,b,frame["click_client"],10)
        self.assertLess(math.dist(value["point"],list(map(float,ordinary))),1e-9)
        self.assertLess(math.dist(rows["aperture-side--4-5-r10"]["point"],list(map(float,weighted))),1e-9)
        self.assertGreater(math.dist(list(map(float,ordinary)),list(map(float,weighted))),1e-6)

    def test_independent_fraction_corpus_regenerates_without_engine_results(self):
        path = ROOT/"crates/viboceros-drafting/src/object_snap/projected_line/mesh_reference.csv"
        self.assertEqual(mesh_near.csv_text(),path.read_text())
        self.assertEqual(len(path.read_text().splitlines()),631)

    def test_retained_provenance_hashes(self):
        provenance = json.loads((ROOT/"docs/point-snaps-provenance.json").read_text())
        for path,digest in provenance["retained_file_sha256"].items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),digest)

    def test_public_topology_order_is_preserved_and_nonfinite_wires_fail_closed(self):
        class Mesh: pass
        mesh = Mesh()
        lines = [SimpleNamespace(From=[2.,3.,4.],To=[5.,6.,7.]),
                 SimpleNamespace(From=[5.,6.,7.],To=[0.,0.,0.])]
        mesh.TopologyEdges = SimpleNamespace(Count=2,EdgeLine=Mock(side_effect=lambda i:lines[i]))
        host = dict(Rhino=SimpleNamespace(Geometry=SimpleNamespace(Mesh=Mesh)),_xyz=lambda p:p)
        self.assertIsNone(probe.topology_wires(object(),host))
        self.assertEqual(probe.topology_wires(mesh,host),[[line.From,line.To] for line in lines])
        self.assertEqual(mesh.TopologyEdges.EdgeLine.call_count,2)
        lines[1].To = [float("nan"),0.,0.]
        with self.assertRaises(ValueError): probe.topology_wires(mesh,host)

    def test_public_wire_picking_is_separate_and_always_disposes_context(self):
        for failure in (None,"ray","transform","query","nonfinite","meshquery","meshpoint","meshdepth"):
            with self.subTest(failure=failure):
                class Matrix:
                    def __getitem__(self,ij): return float(ij[0] == ij[1])
                matrix = Matrix()
                class Mesh: pass
                context = SimpleNamespace(SetPickTransform=Mock(),UpdateClippingPlanes=Mock(),Dispose=Mock())
                def query(line,*args):
                    if args:
                        self.assertEqual(args,("wireframe",))
                        if failure == "meshquery": raise ValueError("mesh query failed")
                        return (True,[float("nan") if failure == "meshpoint" else 1.,2.,3.],
                                float("inf") if failure == "meshdepth" else 0.5,0.75,"Edge",2)
                    if failure == "query": raise ValueError("query failed")
                    return True,0.25,0.5,float("nan") if failure == "nonfinite" else 0.75
                context.PickFrustumTest = query
                def transform(rect):
                    self.assertEqual(rect,(88,188,24,24))
                    if failure == "transform": raise ValueError("transform failed")
                    return matrix
                viewport = SimpleNamespace(GetFrustumLine=lambda x,y:(failure != "ray","ray"),GetPickTransform=transform)
                view = SimpleNamespace(ActiveViewport=viewport)
                factory = Mock(return_value=context)
                factory.MeshPickStyle = SimpleNamespace(WireframePicking="wireframe")
                host = dict(Rhino=SimpleNamespace(
                    Input=SimpleNamespace(Custom=SimpleNamespace(PickContext=factory,PickStyle=SimpleNamespace(PointPick="point"))),
                    Geometry=SimpleNamespace(Mesh=Mesh,Line=lambda a,b:(a,b))),
                    System=SimpleNamespace(Drawing=SimpleNamespace(Rectangle=lambda *args:args)),_point=lambda p:p,_xyz=lambda p:p)
                result = dict(frame=dict(click_client=[100,200]),topology_wires=[None,[[[0.,0.,0.],[1.,2.,3.]]]])
                geometries = [object(),Mesh()]
                if failure:
                    with self.assertRaises(ValueError): probe.wire_pick_diagnostics(view,result,12,host,geometries)
                else:
                    value = probe.wire_pick_diagnostics(view,result,12,host,geometries)
                    self.assertEqual(value["sources"],[None,[dict(t=0.25,depth=0.5,distance=0.75)]])
                    self.assertEqual(value["meshes"],[None,dict(point=[1.,2.,3.],depth=0.5,distance=0.75,flag="Edge",index=2)])
                    self.assertEqual(context.PickLine,"ray")
                    context.SetPickTransform.assert_called_once_with(matrix)
                    context.UpdateClippingPlanes.assert_called_once()
                context.Dispose.assert_called_once()

    def test_request_and_source_validation_precedes_host_access(self):
        base = request()
        probe.validate_request(base)
        for invalid in (None,[],False):
            with self.assertRaises(ValueError): probe.validate_request(invalid)
        operation = base["operations"][0]
        for changes in [dict(aim=None), dict(aim=[True,0,0]), dict(aim=[float("inf"),0,0]),
                        dict(aim=[10**400,0,0]), dict(offset=[33,0]), dict(offset=[0.5,0]),
                        dict(offset=[True,0]), dict(snap_to_meshes=None), dict(snap_to_meshes=1),
                        dict(capture_radius=0), dict(capture_radius=65), dict(capture_radius=True), dict(capture_radius=12.5),
                        dict(pick_diagnostics=None), dict(pick_diagnostics=1),
                        dict(input_settle_ms=True), dict(input_settle_ms=-1), dict(input_settle_ms=1001), dict(input_settle_ms=0.5),
                        dict(input_detour=[1,0]),
                        dict(persistent_snaps=["Near","Near"]), dict(persistent_snaps=[{}]),
                        dict(persistent_snaps=["Near _Delete"]), dict(persistent_snaps=None),
                        dict(bounds=[[2,2,2],[1,1,1]]), dict(view="Perspective _Delete"),
                        dict(id="bad\nPICK"), dict(extra="ignored"), dict(sources=[]),
                        dict(sources=[dict(type="line",start=[0,0,0],end=[1,2])])]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                probe.run(dict(operation, **changes), None, {})
        for detour in (None, [0,0], [1,1], [2,0], [True,0], [1.0,0], [1], "left"):
            with self.subTest(detour=detour), self.assertRaises(ValueError):
                probe.validate(dict(operation,input_settle_ms=250,input_detour=detour))
        for detour in ([1,0],[-1,0],[0,1],[0,-1]):
            probe.validate(dict(operation,input_settle_ms=250,input_detour=detour))
        mesh_op = copy.deepcopy(base["operations"][2])
        for face in ([0,1,4], [0,1,True], [0,0,2], [0,1], None):
            mesh_op["sources"][0]["faces"] = [face]
            with self.subTest(face=face), self.assertRaises(ValueError): probe.validate(mesh_op)
        for changes in [dict(protocol_version=True), dict(iterations=True), dict(iterations=2),
                        dict(operations=[]), dict(operations=[operation]*2), dict(operations=[{}])]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                probe.validate_request(dict(base, **changes))

    def exercise_pick(self, failure=None):
        class Event:
            def __init__(self): self.handlers = []
            def __iadd__(self, handler): self.handlers.append(handler); return self
            def __isub__(self, handler): self.handlers.remove(handler); return self
            def fire(self):
                for handler in self.handlers: handler(None,None)
        timer = SimpleNamespace(Tick=Event(), Start=Mock(), Stop=Mock(), Dispose=Mock())
        pixel = lambda x,y: SimpleNamespace(X=x,Y=y)
        viewport = SimpleNamespace(WorldToClient=lambda p: pixel(100.7,200.2), Size=SimpleNamespace(Width=500,Height=500))
        view = SimpleNamespace(ActiveViewport=viewport, ClientToScreen=lambda p: pixel(p.X+10,p.Y+20))
        document = SimpleNamespace(InGetPoint=False, Views=SimpleNamespace(ActiveView=view))
        output = mock_open()
        if failure == "write": output.side_effect = OSError("write failed")
        reference = SimpleNamespace(ObjectId="owned",Dispose=Mock(),GeometryComponentIndex=SimpleNamespace(ComponentIndexType="MeshTopologyEdge",Index=2))
        getter = SimpleNamespace(SetCommandPrompt=Mock(), Dispose=Mock(), OsnapEventType="Near",
                                 Point=lambda: [3.,-2.,7.] if failure != "point" else [float("nan"),0,0],
                                 PointOnObject=lambda: reference)
        def get():
            timer.Tick.fire()
            self.assertEqual(output.call_count,0,"wait for the real point prompt")
            document.InGetPoint = True
            timer.Tick.fire()
            calls = output.call_count
            timer.Tick.fire()
            self.assertEqual(output.call_count,calls,"never repeat a click or abort")
            if failure == "get": raise ValueError("get failed")
            return "Cancel" if failure == "cancel" else "Point"
        getter.Get = get
        rhino = SimpleNamespace(RhinoDoc=SimpleNamespace(ActiveDoc=document),
                                Input=SimpleNamespace(Custom=SimpleNamespace(GetPoint=lambda:getter), GetResult=SimpleNamespace(Point="Point")))
        host = dict(Rhino=rhino,System=SimpleNamespace(Drawing=SimpleNamespace(Point=pixel)),
                    __file__="/owned/worker.py",_point=lambda p:p,_xyz=lambda p:p)
        modules = {"clr":SimpleNamespace(AddReference=Mock()),"System.Windows.Forms":SimpleNamespace(Timer=lambda:timer)}
        capture = Mock(return_value={"camera":"actual"})
        if failure == "camera": capture.side_effect = ValueError("bad matrix")
        with patch.dict(sys.modules,modules), patch("builtins.open",output), patch.object(probe.viewport_capture,"capture",capture):
            if failure:
                with self.assertRaises(ValueError): probe.pick(request()["operations"][0],host)
            else:
                value, source = probe.pick(request()["operations"][0],host)
                self.assertEqual(source,"owned")
                self.assertEqual(value,dict(point=[3.,-2.,7.],kind="Near",frame={"camera":"actual"},
                                           component=dict(type="MeshTopologyEdge",index=2)))
                output().write.assert_called_once_with("PICK @point:perspective-line-near-on 110 220\n")
                reference.Dispose.assert_called_once()
        self.assertEqual(timer.Tick.handlers,[])
        timer.Stop.assert_called()
        timer.Dispose.assert_called_once()
        getter.Dispose.assert_called_once()

    def test_driver_waits_for_prompt_and_records_public_kind_point_and_owner(self):
        self.exercise_pick()

    def test_driver_fails_closed_and_disposes_on_io_camera_cancel_and_get_failures(self):
        for failure in ("write","camera","cancel","get","point"):
            with self.subTest(failure=failure): self.exercise_pick(failure)

    def test_owned_sources_and_view_restore_after_setup_pick_and_cleanup_failures(self):
        for failure in (None,"source","projection","fit","pick","owner","topology","diagnostics","delete"):
            with self.subTest(failure=failure):
                created, table, deleted = [], {}, []
                class Curve:
                    IsValid = True
                    PointAtStart, PointAtEnd = [2.,-2.,7.],[8.,-2.,7.]
                    def __init__(self): self.Dispose = Mock()
                def source(definition,tolerance):
                    if failure == "source" and created: raise ValueError("source failed")
                    curve = Curve(); created.append(curve); return curve
                def add(curve):
                    key = "owned-"+str(len(table)); table[key] = SimpleNamespace(Geometry=curve); return key
                def delete(key,quiet):
                    deleted.append(key)
                    if failure == "delete": return False
                    del table[key]; return True
                objects = SimpleNamespace(GetObjectList=lambda s:list(table.values()),AddCurve=add,
                                          FindId=lambda key:table[key],Delete=delete)
                viewport = SimpleNamespace(Name="original",CameraTarget=[11,12,13],
                    SetProjection=Mock(return_value=failure != "projection"),
                    ZoomBoundingBox=Mock(return_value=failure != "fit"),
                    SetViewProjection=Mock(return_value=True),SetCameraTarget=Mock())
                saved = SimpleNamespace(Dispose=Mock())
                view = SimpleNamespace(ActiveViewport=viewport)
                document = SimpleNamespace(Objects=objects,Views=SimpleNamespace(ActiveView=view,Redraw=Mock()))
                rhino = SimpleNamespace(RhinoDoc=SimpleNamespace(ActiveDoc=document),
                    Geometry=SimpleNamespace(Mesh=type("Mesh",(),{}),BoundingBox=lambda a,b:[a,b]),
                    Display=SimpleNamespace(DefinedViewportProjection=SimpleNamespace(Perspective=1)),
                    DocObjects=SimpleNamespace(ObjectEnumeratorSettings=SimpleNamespace,ViewportInfo=lambda vp:saved))
                host = dict(Rhino=rhino,System=SimpleNamespace(Guid=SimpleNamespace(Empty="empty")),
                            _point=lambda p:p,_xyz=lambda p:p,_object_source=source)
                entered, restored = [], []
                @contextmanager
                def environment(operation,host,radius):
                    self.assertEqual(radius,12)
                    entered.append(True)
                    try: yield dict(before=False,requested=True,restored=False)
                    finally: restored.append(True)
                def pick(operation,host):
                    viewport.Name = "changed"
                    if failure == "pick": raise ValueError("pick failed")
                    return dict(point=[3.,-2.,7.],kind="Near"), "foreign" if failure == "owner" else "owned-0"
                op = copy.deepcopy(request()["operations"][0])
                op["sources"] *= 2
                if failure == "diagnostics": op["pick_diagnostics"] = True
                def topology(geometry,host):
                    if failure == "topology": raise ValueError("topology query failed")
                    return None
                with patch.object(probe,"pick",side_effect=pick), patch.object(probe.snap_environment,"environment",environment), patch.object(probe,"topology_wires",side_effect=topology), patch.object(probe,"wire_pick_diagnostics",side_effect=ValueError("diagnostic failed")):
                    if failure:
                        with self.assertRaises(ValueError): probe.run(op,None,host)
                    else:
                        value,elapsed = probe.run(op,None,host)
                        self.assertEqual(elapsed,0)
                        self.assertEqual(value["source"],0)
                        self.assertEqual(value["before"],value["after"])
                        self.assertEqual(value["topology_wires"],[None,None])
                self.assertEqual(len(deleted),len(created))
                for geometry in created: geometry.Dispose.assert_called_once()
                saved.Dispose.assert_called_once()
                viewport.SetViewProjection.assert_called_once_with(saved,False)
                viewport.SetCameraTarget.assert_called_once_with([11,12,13],False)
                self.assertEqual(viewport.Name,"original")
                self.assertEqual(entered,restored)

"""Test the worker's host-independent orchestration with a simulated document."""

import importlib.util
import math
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch


class RhinoWorkerTests(unittest.TestCase):
    def test_canonical_mesh_record_preserves_winding_duplicates_and_unrounded_points(self):
        original = {"vertices": [[0,0,0],[1,0,0],[1,1,0],[0,1,0],[0,0,0]], "faces": [[0,1,2,3]]}
        reordered = {"vertices": list(reversed(original["vertices"])), "faces": [[2,1,4,3]]}
        a = self.worker._canonical_mesh_record(original)
        self.assertEqual(a, self.worker._canonical_mesh_record(reordered))
        reversed_face = dict(original, faces=[[0,3,2,1]])
        self.assertNotEqual(a, self.worker._canonical_mesh_record(reversed_face))
        changed = dict(original, vertices=[[0,0,1e-12]] + original["vertices"][1:])
        self.assertNotEqual(a, self.worker._canonical_mesh_record(changed))
        self.assertEqual(len(a["vertices"]), 5)

    def test_plane_primitive_macros_use_world_points_and_actual_option_names(self):
        operation = {"points": [[1,2,3],[4,5,6]], "value": None}
        with patch.object(self.worker, "_command_point", side_effect=lambda p: ",".join(str(x) for x in p)):
            self.assertEqual(self.worker._plane_primitive_script(dict(operation, primitive="Circle")), "_Circle w1,2,3 w4,5,6")
            self.assertEqual(self.worker._plane_primitive_script(dict(operation, primitive="Polygon")), "_Polygon _NumSides=5 _Mode=_Inscribed w1,2,3 w4,5,6")
            self.assertEqual(self.worker._plane_primitive_script(dict(operation, primitive="MeshBox", value=-4)), "_MeshBox _XCount=2 _YCount=3 _ZCount=2 w1,2,3 w4,5,6 -4")
        for invalid in [dict(operation, primitive="Delete"), dict(operation, primitive="Circle _Delete"),
                        dict(operation, primitive="Rectangle", value=3), dict(operation, primitive="Circle", points=[])]:
            with self.assertRaises(ValueError):
                self.worker._plane_primitive_script(invalid)

    def test_plane_primitive_extrusion_record_disposes_its_temporary_brep(self):
        class Extrusion:
            IsValid = True
            def ToBrep(self):
                return brep
        brep = SimpleNamespace(Dispose=Mock())
        self.worker.Rhino.Geometry = SimpleNamespace(Extrusion=Extrusion)
        with patch.object(self.worker, "_interchange_brep_record", side_effect=ValueError("record failed")):
            with self.assertRaisesRegex(ValueError, "record failed"):
                self.worker._plane_primitive_record(Extrusion())
        brep.Dispose.assert_called_once_with()

    def test_point_input_script_accepts_coordinates_but_not_commands(self):
        tokens = ["0", "w1,2,3", "@w2<45", "r3<20<30", "wr1e-3,2.5,0"]
        self.assertEqual(self.worker._point_input_script(tokens), "_Polyline " + " ".join(tokens) + " _Enter")
        for invalid in [[], ["0"], ["0"] * 257, ["0", "_Delete"], ["0", "1,2 _Enter"],
                        ["0", "1,2\n_Delete"], ["0", "1;2"], ["0", "r"], ["0", None], ["0", "1" * 513]]:
            with self.subTest(invalid=invalid):
                with self.assertRaises(ValueError):
                    self.worker._point_input_script(invalid)

    def test_point_input_probe_restores_plane_selection_and_owned_outputs_on_failure(self):
        for failed in [False, True]:
            with self.subTest(failed=failed):
                original_plane = object()
                current_plane = [original_plane]
                viewport = SimpleNamespace(
                    ConstructionPlane=lambda: current_plane[0],
                    SetConstructionPlane=lambda plane: current_plane.__setitem__(0, plane))
                selected = {"existing"}
                existing = SimpleNamespace(Id="existing", IsSelected=lambda _: "existing" in selected)
                objects = {"existing": existing}
                table = SimpleNamespace(
                    GetObjectList=lambda _: list(objects.values()),
                    UnselectAll=selected.clear, Select=selected.add,
                    Delete=lambda object_id, _: objects.pop(object_id))
                self.document.Objects = table
                self.document.Views = SimpleNamespace(ActiveView=SimpleNamespace(ActiveViewport=viewport))
                self.worker.Rhino.DocObjects = SimpleNamespace(ObjectEnumeratorSettings=SimpleNamespace)
                self.worker.Rhino.Geometry = SimpleNamespace(Plane=lambda *args: SimpleNamespace(IsValid=True))
                self.worker.Rhino.RhinoApp.RunScript = Mock()
                points = [SimpleNamespace(X=0.0,Y=0.0,Z=0.0), SimpleNamespace(X=1.0,Y=2.0,Z=0.0)]
                def run(script, verify):
                    self.assertTrue(verify)
                    objects["created"] = SimpleNamespace(Id="created", Geometry=SimpleNamespace(TryGetPolyline=lambda: (True, points)))
                    if failed:
                        raise ValueError("failed coordinate command")
                    return True
                operation = {"points": ["0", "1,2"], "origin": [0,0,0], "x_axis": [1,0,0], "y_axis": [0,1,0]}
                with patch.object(self.worker, "_point", side_effect=lambda value: value), patch.object(
                    self.worker, "_vector", side_effect=lambda value: value), patch.object(self.worker, "_run_surface_script", side_effect=run):
                    if failed:
                        with self.assertRaisesRegex(ValueError, "failed coordinate command"):
                            self.worker._point_input(operation)
                    else:
                        value, elapsed = self.worker._point_input(operation)
                        self.assertEqual(value, {"points": [[0.0,0.0,0.0],[1.0,2.0,0.0]]})
                        self.assertEqual(elapsed, 0)
                self.assertIs(current_plane[0], original_plane)
                self.assertEqual(set(objects), {"existing"})
                self.assertEqual(selected, {"existing"})
                self.worker.Rhino.RhinoApp.RunScript.assert_called_once_with("!", False)

    def test_loft_samples_are_unrounded_and_use_native_domains(self):
        for enabled in [False, True]:
            domains = [SimpleNamespace(ParameterAt=lambda s: 2.0 + 3.0 * s),
                       SimpleNamespace(ParameterAt=lambda s: -7.0 + 13.0 * s)]
            surface = SimpleNamespace(
                Domain=lambda axis: domains[axis],
                PointAt=Mock(side_effect=lambda u, v: SimpleNamespace(X=u, Y=v, Z=0.123456789012345)))
            with patch.object(self.worker, "_nurbs_surface_definition", return_value={"degree": [1, 2]}):
                value = self.worker._loft_surface_record(surface, {"sample_geometry": enabled})
            if enabled:
                self.assertEqual(len(value["samples"]), 289)
                self.assertEqual(value["samples"][0], [2.0, -7.0, 0.123456789012345])
                self.assertEqual(value["samples"][17], [2.1875, -7.0, 0.123456789012345])
                self.assertEqual(value["samples"][-1], [5.0, 6.0, 0.123456789012345])
            else:
                self.assertNotIn("samples", value)
                surface.PointAt.assert_not_called()

    def test_sweep_macro_sets_actual_script_options_instead_of_dialog_labels(self):
        for blend, name in [(0, "Local"), (1, "Global")]:
            for refit, value in [(False, "No"), (True, "Yes")]:
                script = self.worker._sweep1_macro({"blend": blend, "refit_rail": refit}, ["rail", "a", "b"])
                self.assertTrue(script.startswith("_-Sweep1 '_-SelID rail '_-SelID a '_-SelID b _Enter"))
                for expected in ["_Style=_Freeform", "_Simplify=_None", "_Closed=_No",
                                 "_ShapeBlending=_" + name, "_RefitRail=_" + value]:
                    self.assertIn(expected, script)
                self.assertNotIn("FrameStyle", script)
                self.assertNotIn("GlobalShapeBlending", script)

    def test_surface_script_verification_rejects_ignored_options_even_on_success(self):
        for rejected in [False, True]:
            for truncated in [False, True]:
                with self.subTest(rejected=rejected, truncated=truncated):
                    app = SimpleNamespace(CommandHistoryWindowText="old Unknown command: unrelated\n")
                    def run(script, echo):
                        self.assertTrue(echo)
                        app.CommandHistoryWindowText = ("" if truncated else app.CommandHistoryWindowText) + (
                            "Command: _-Sweep1\n" + ("Unknown command: _BadOption\n" if rejected else "Sweep complete\n"))
                        return True
                    app.RunScript = run
                    self.worker.Rhino.RhinoApp = app
                    if rejected:
                        with self.assertRaisesRegex(ValueError, "rejected an option"):
                            self.worker._run_surface_script("_-Sweep1 _Enter", True)
                    else:
                        self.assertTrue(self.worker._run_surface_script("_-Sweep1 _Enter", True))

    def test_sweep_probe_disposes_inputs_and_replaced_results_on_every_exit(self):
        for failure in [None, "input", "build", "record", "empty"]:
            with self.subTest(failure=failure):
                curves = [SimpleNamespace(Dispose=Mock()) for _ in range(2)]
                breps = [SimpleNamespace(Dispose=Mock()) for _ in range(3)]
                created = []
                def build(*args):
                    if failure == "build" and created:
                        raise ValueError("sweep failed")
                    if failure == "empty":
                        return None
                    brep = breps[len(created)]
                    created.append(brep)
                    return [brep]
                self.worker.Rhino.Geometry = SimpleNamespace(
                    Brep=SimpleNamespace(CreateFromSweep=build),
                    Point3d=SimpleNamespace(Unset=None), Vector3d=SimpleNamespace(Unset=None),
                    SweepFrame=object(), SweepBlend=object(), SweepMiter=object(), SweepRebuild=object())
                self.worker.System.Enum = SimpleNamespace(ToObject=lambda kind, value: value)
                inputs = [curves[0], ValueError("bad section")] if failure == "input" else curves
                with patch.object(self.worker, "_join_close_input", side_effect=inputs), patch.object(
                    self.worker, "_sweep1_record", return_value=[{"samples": []}],
                    side_effect=ValueError("bad result") if failure == "record" else None):
                    operation = {"rail": {}, "sections": [{}]}
                    if failure in ["input", "build", "record"]:
                        with self.assertRaises(ValueError):
                            self.worker._sweep1(operation, 2, {"absolute": 1e-7})
                    else:
                        value, _ = self.worker._sweep1(operation, 2, {"absolute": 1e-7})
                        self.assertEqual(value, [] if failure == "empty" else [{"samples": []}])
                for curve in curves[:1 if failure == "input" else 2]:
                    curve.Dispose.assert_called_once_with()
                for brep in created:
                    brep.Dispose.assert_called_once_with()

    def test_curve_frames_validate_results_and_dispose_sources_on_every_exit(self):
        vector = lambda x,y,z: SimpleNamespace(X=x,Y=y,Z=z)
        first = SimpleNamespace(Origin=vector(0,0,0),XAxis=vector(1,0,0),YAxis=vector(0,1,0),ZAxis=vector(0,0,1))
        last = SimpleNamespace(Origin=vector(1,2,3),XAxis=vector(0,1,0),YAxis=vector(-1,0,0),ZAxis=vector(0,0,1))
        for failure in [None,"null","count","compute","record","parameters"]:
            frames = None if failure == "null" else [first] if failure == "count" else [first,last]
            curve = SimpleNamespace(Domain=SimpleNamespace(T0=0.0,T1=1.0),Dispose=Mock(),
                GetPerpendicularFrames=Mock(return_value=frames,
                    side_effect=ValueError("frame failure") if failure == "compute" else None))
            with patch.object(self.worker,"_join_close_input",return_value=curve), patch.object(
                self.worker,"_xyz",side_effect=ValueError("record failure") if failure == "record" else lambda p:[p.X,p.Y,p.Z]):
                operation = {"curve":{},"parameters":[1.0,0.0] if failure == "parameters" else [0.0,1.0]}
                if failure not in [None,"null"]:
                    with self.assertRaises(ValueError): self.worker._curve_frames(operation,2)
                else:
                    value,_ = self.worker._curve_frames(operation,2)
                    self.assertEqual(value["available"],failure is None)
                    if failure is None:
                        self.assertEqual(value["samples"][1]["rotation"],[[0,-1,0],[1,0,0],[0,0,1]])
                        self.assertEqual(value["samples"][1]["point"],[1,2,3])
                    else: self.assertEqual(value["samples"],[])
                    self.assertEqual(curve.GetPerpendicularFrames.call_count,3)
            curve.Dispose.assert_called_once_with()

    def test_curvature_command_disposes_attributes_and_geometry_and_cleans_only_owned_objects(self):
        for failure in [None, "add", "command", "report", "record", "point", "marker"]:
            objects = {100: SimpleNamespace(Id=100)}
            attributes = []
            geometry = SimpleNamespace(Dispose=Mock(), DataCRC=lambda seed: 7)
            def attr():
                value = SimpleNamespace(Name=None, GroupCount=0, Dispose=Mock())
                attributes.append(value)
                return value
            def add(g, a):
                if failure == "add": raise ValueError("add failed")
                objects[1] = SimpleNamespace(Id=1, Geometry=g, Attributes=a, IsSelected=lambda sub: True)
                return 1
            def command(macro, echo):
                self.assertEqual(macro, "_Curvature _MarkCurvature=Yes 2,0,0 _Enter")
                self.worker.Rhino.RhinoApp.CommandHistoryWindowText += (
                    "no measurement\n" if failure == "report" else "Radius of curvature is infinite.\n")
                self.worker.Rhino.RhinoApp.CommandHistoryWindowText = self.worker.Rhino.RhinoApp.CommandHistoryWindowText[-180:]
                objects[2] = SimpleNamespace(Id=2, Geometry=object(),
                    Attributes=SimpleNamespace(Name=None, GroupCount=0), IsSelected=lambda sub: False)
                return failure != "command"
            self.document.Objects = SimpleNamespace(GetObjectList=lambda settings: list(objects.values()),
                UnselectAll=lambda: None, AddCurve=add, Select=lambda i: None,
                FindId=objects.get, Delete=lambda i, quiet: objects.pop(i))
            self.worker.Rhino.DocObjects = SimpleNamespace(ObjectEnumeratorSettings=SimpleNamespace, ObjectAttributes=attr)
            self.worker.Rhino.RhinoApp.RunScript = command
            self.worker.Rhino.RhinoApp.CommandHistoryWindowText = "before\n"
            def write_line(line):
                if failure != "marker":
                    self.worker.Rhino.RhinoApp.CommandHistoryWindowText += line + "\n"
            self.worker.Rhino.RhinoApp.WriteLine = write_line
            self.worker.System.Guid = SimpleNamespace(Empty=-1, NewGuid=lambda: len(attributes))
            with patch.object(self.worker, "_nurbs_curve_from_definition", return_value=geometry), patch.object(
                self.worker, "_command_point", return_value="2,0,0", side_effect=ValueError("bad point") if failure == "point" else None
            ), patch.object(self.worker, "_curvature_marker", return_value={"kind": "point", "point": [2,0,0]},
                            side_effect=ValueError("record failed") if failure == "record" else None):
                operation = {"curve": {}, "point": [2,0,0], "mark": True}
                if failure:
                    with self.assertRaises(ValueError): self.worker._curvature_command(operation, 2, {"absolute":1e-9})
                else:
                    result, _ = self.worker._curvature_command(operation, 2, {"absolute":1e-9})
                    self.assertTrue(result["reported"])
                    self.assertTrue(result["source_geometry_unchanged"])
                    self.assertTrue(result["source_selected"])
                    self.assertEqual(result["source_name"], "curvature-source")
                    self.assertEqual(len(result["outputs"]), 1)
            self.assertEqual(set(objects), {100})
            if failure == "point": geometry.Dispose.assert_not_called()
            else: geometry.Dispose.assert_called_once_with()
            for value in attributes: value.Dispose.assert_called_once_with()

    def test_curvature_uses_prepared_quadrants_and_sign_invariant_shape_operator(self):
        vector = lambda x,y,z: SimpleNamespace(X=x,Y=y,Z=z)
        interval = lambda a,b: SimpleNamespace(T0=a,T1=b)
        self.worker.Rhino.Geometry = SimpleNamespace(Interval=interval)
        curvature = SimpleNamespace(Point=vector(1,2,3), Normal=vector(0,0,1), Gaussian=-2.0, Mean=0.5,
            Kappa=lambda i: [2.0,-1.0][i], Direction=lambda i: [vector(-1,0,0),vector(0,-1,0)][i])
        for failure in [False,True]:
            piece = SimpleNamespace(Dispose=Mock(), CurvatureAt=Mock(return_value=curvature,
                side_effect=ValueError("evaluation failed") if failure else None))
            source = SimpleNamespace(Domain=lambda axis: interval(0.0,2.0), Dispose=Mock(),
                Trim=Mock(return_value=piece), CurvatureAt=Mock(return_value=None))
            operation = {"surface":{}, "samples":[{"parameter":[1.0,1.0],"side_u":"left"}, {"parameter":[0.0,0.0]}]}
            with patch.object(self.worker, "_nurbs_surface_from_definition", return_value=source):
                if failure:
                    with self.assertRaises(ValueError): self.worker._surface_jets(operation,1,True)
                else:
                    value,_ = self.worker._surface_jets(operation,1,True)
                    self.assertEqual(value["samples"][0]["shape_operator"], [[2.0,0.0,0.0],[0.0,-1.0,0.0],[0.0,0.0,0.0]])
                    self.assertEqual(value["samples"][0]["principal"], [2.0,-1.0])
                    self.assertEqual(value["samples"][1], {"available":False,"parameter":[0.0,0.0]})
            piece.Dispose.assert_called_once_with()
            source.Dispose.assert_called_once_with()

    def test_point_grid_api_disposes_results_on_success_null_and_failures(self):
        for control in [False, True]:
            for failure in [None, "build", "record", "null"]:
                results = []
                def build(*args):
                    if failure == "build" and results:
                        raise ValueError("build failed")
                    if failure == "null":
                        return None
                    result = SimpleNamespace(Dispose=Mock())
                    results.append(result)
                    return result
                self.worker.Rhino.Geometry = SimpleNamespace(NurbsSurface=SimpleNamespace(
                    CreateFromPoints=build, CreateThroughPoints=build))
                with patch.object(self.worker, "_point", side_effect=lambda p: p), patch.object(
                    self.worker, "_surface_grid_record", return_value={"valid": True},
                    side_effect=ValueError("record failed") if failure == "record" else None
                ):
                    operation = {"count": [2, 2], "points": [[0, 0, 0]]*4, "control": control}
                    if failure in ["build", "record"]:
                        with self.assertRaises(ValueError):
                            self.worker._surface_grid(operation, 2)
                    else:
                        value, _ = self.worker._surface_grid(operation, 2)
                        self.assertEqual(value, None if failure == "null" else {"valid": True})
                        self.assertEqual(len(results), 0 if failure == "null" else 3)
                for result in results:
                    result.Dispose.assert_called_once_with()

    def test_point_grid_command_records_every_face_and_cleans_only_owned_objects(self):
        for control, failure in [(c, f) for c in [False, True]
                                 for f in [None, "add", "command", "record", "degree"]]:
            objects = {0: SimpleNamespace(Id=0)}
            class Point:
                def __init__(self, p): self.Location = p
            class Cloud:
                def GetPoints(self): return [SimpleNamespace(X=1, Y=2, Z=3)]
            class Brep:
                IsValid = True
                Vertices = SimpleNamespace(Count=6)
                Edges = SimpleNamespace(Count=7)
                Faces = [SimpleNamespace(index=1, OrientationIsReversed=True),
                         SimpleNamespace(index=0, OrientationIsReversed=False)]
            def obj(index, geometry, selected=False):
                return SimpleNamespace(Id=index, Geometry=geometry,
                    Attributes=SimpleNamespace(Name=None, GroupCount=0), IsSelected=lambda sub: selected)
            def add(p):
                if failure == "add": raise ValueError("add failed")
                objects[1] = obj(1, Point(p), True)
                return 1
            def command(macro, echo):
                if control:
                    self.assertIn("_SrfControlPtGrid _KeepPoints=Yes _Degree=2 3 _Degree=2 3 ", macro)
                    self.assertNotIn("_DegreeU", macro)
                    self.assertNotIn("_Closed", macro)
                else:
                    for option in ["_DegreeU=2", "_DegreeV=2", "_ClosedU=No", "_ClosedV=No"]:
                        self.assertIn(option, macro)
                    self.assertNotIn("_Degree=", macro)
                objects[2], objects[3] = obj(2, Brep()), obj(3, Cloud())
                return failure != "command"
            def record(face, operation, constraints):
                if failure == "record": raise ValueError("record failed")
                self.assertFalse(constraints)
                return {"surface": {"degree": [1, 1] if failure == "degree" else [2, 2],
                                    "domain_u": [0, 1], "domain_v": [face.index, face.index+1]}}
            self.document.Objects = SimpleNamespace(GetObjectList=lambda settings: list(objects.values()),
                UnselectAll=lambda: None, AddPoint=add, Select=lambda i: None,
                FindId=objects.get, Delete=lambda i, quiet: objects.pop(i))
            self.worker.Rhino.DocObjects = SimpleNamespace(ObjectEnumeratorSettings=SimpleNamespace)
            self.worker.Rhino.Geometry = SimpleNamespace(Point=Point, PointCloud=Cloud, Brep=Brep,
                Point3d=lambda x,y,z: SimpleNamespace(X=x, Y=y, Z=z))
            self.worker.Rhino.RhinoApp.RunScript = command
            self.worker.Rhino.RhinoApp.CommandHistoryWindowText = "test macro failure"
            self.worker.System.Guid = SimpleNamespace(Empty=-1)
            operation = {"count": [3, 3], "degree": [2, 2], "points": [[0, 0, 0]]*9,
                         "keep_points": True, "control": control}
            with patch.object(self.worker, "_surface_grid_record", side_effect=record):
                if failure:
                    with self.assertRaises(ValueError): self.worker._surface_grid_command(operation, 2)
                else:
                    value, _ = self.worker._surface_grid_command(operation, 2)
                    self.assertEqual(len(value["outputs"][0]["faces"]), 2)
                    self.assertEqual(value["outputs"][0]["face_reversed"], [False, True])
                    self.assertEqual(value["points"][0]["kind"], "point_cloud")
                    self.assertTrue(value["sentinel_selected"])
            self.assertEqual(set(objects), {0})

    def test_point_grid_command_rejects_counts_that_would_leave_rhino_at_a_prompt(self):
        self.worker.Rhino.RhinoApp.RunScript = Mock()
        for operation in [
            {"count": [7, 6], "degree": [11, 11], "points": []},
            {"count": [4, 4], "points": [[0, 0, 0]]*15},
            {"count": [4, 4], "points": [[0, 0, 0]]*17},
            {"count": [4], "points": []},
            {"count": [4, 4], "degree": [3], "points": []},
            {"count": [4.5, 4], "points": []},
            {"count": [257, 4], "points": []},
        ]:
            with self.assertRaises(ValueError):
                self.worker._surface_grid_command(operation, 1)
        self.worker.Rhino.RhinoApp.RunScript.assert_not_called()

    def test_edge_surface_disposes_owned_geometry_on_success_null_and_failure(self):
        for failure in [None, "source", "build", "record", "null"]:
            sources, results = [], []
            def source(definition):
                if failure == "source" and sources:
                    raise ValueError("source failed")
                curve = SimpleNamespace(Dispose=Mock())
                sources.append(curve)
                return curve
            def build(curves):
                if failure == "build" and results:
                    raise ValueError("build failed")
                if failure == "null":
                    return None
                brep = SimpleNamespace(Dispose=Mock())
                results.append(brep)
                return brep
            self.worker.Rhino.Geometry = SimpleNamespace(Brep=SimpleNamespace(CreateEdgeSurface=build))
            with patch.object(self.worker, "_nurbs_curve_from_definition", side_effect=source), patch.object(
                self.worker, "_edge_surface_record", return_value={"valid": True},
                side_effect=ValueError("record failed") if failure == "record" else None
            ):
                if failure in ["source", "build", "record"]:
                    with self.assertRaises(ValueError):
                        self.worker._edge_surface({"curves": [{}, {}]}, 2)
                else:
                    value, _ = self.worker._edge_surface({"curves": [{}, {}]}, 2)
                    self.assertEqual(value, None if failure == "null" else {"valid": True})
                    self.assertEqual(len(results), 0 if failure == "null" else 3)
            for item in sources + results:
                item.Dispose.assert_called_once_with()

    def test_edge_surface_comparison_owns_its_copy_and_checks_original_geometry(self):
        for failure in [None, "lower", "elevate", "geometry", "record"]:
            def point(u, v, shift=0.0):
                return SimpleNamespace(X=u, Y=v, Z=u*v+shift)
            canonical = SimpleNamespace(
                Dispose=Mock(), Degree=lambda axis: 2,
                IncreaseDegreeU=Mock(return_value=failure != "elevate"),
                IncreaseDegreeV=Mock(return_value=True),
                PointAt=lambda u,v: point(u,v,1e-4 if failure == "geometry" else 0.0))
            face = SimpleNamespace(
                Domain=lambda axis: SimpleNamespace(ParameterAt=lambda t: t),
                PointAt=point, OrientationIsReversed=False,
                ToNurbsSurface=Mock(return_value=canonical))
            brep = SimpleNamespace(Faces=[face], IsValid=True,
                                   Vertices=SimpleNamespace(Count=4), Edges=SimpleNamespace(Count=4), Trims=[])
            with patch.object(self.worker, "_nurbs_surface_definition", return_value={"degree": [3, 2]},
                              side_effect=ValueError("record failed") if failure == "record" else None):
                if failure:
                    with self.assertRaises(ValueError):
                        self.worker._edge_surface_record(brep, [1, 2] if failure == "lower" else [3, 2])
                else:
                    result = self.worker._edge_surface_record(brep, [3, 2])
                    self.assertEqual(len(result["samples"][0]), 169)
                    self.assertEqual(result["surfaces"], [{"degree": [3, 2]}])
            face.ToNurbsSurface.assert_called_once_with()
            canonical.Dispose.assert_called_once_with()

    def test_loft_command_cleans_only_owned_objects_and_disposes_attributes(self):
        for failure in [None, "add", "command", "record"]:
            objects = {0: SimpleNamespace(Id=0)}
            attributes = []
            next_id = [1]
            def attrs():
                value = SimpleNamespace(Name=None, GroupCount=0, Dispose=Mock())
                attributes.append(value)
                return value
            def add(curve, attribute):
                if failure == "add" and len(attributes) == 2:
                    raise ValueError("add failed")
                index = next_id[0]
                next_id[0] += 1
                objects[index] = SimpleNamespace(Id=index, IsSelected=lambda sub: True)
                return index
            def command(macro, echo):
                index = next_id[0]
                next_id[0] += 1
                objects[index] = SimpleNamespace(
                    Id=index, IsSelected=lambda sub: False,
                    Attributes=SimpleNamespace(Name=None, GroupCount=0),
                    Geometry=SimpleNamespace(IsValid=True, Vertices=SimpleNamespace(Count=4),
                        Edges=SimpleNamespace(Count=4), Faces=[SimpleNamespace(OrientationIsReversed=False)]))
                return failure != "command"
            self.document.Objects = SimpleNamespace(
                GetObjectList=lambda settings: list(objects.values()), UnselectAll=lambda: None,
                AddCurve=add, Select=lambda index: None, FindId=objects.get,
                Delete=lambda index, quiet: objects.pop(index))
            self.worker.Rhino.DocObjects = SimpleNamespace(
                ObjectEnumeratorSettings=SimpleNamespace, ObjectAttributes=attrs)
            self.worker.Rhino.RhinoApp.RunScript = command
            self.worker.Rhino.RhinoApp.CommandHistoryWindowText = "failed test command"
            self.worker.System.Guid = SimpleNamespace(Empty=-1)
            sources = [SimpleNamespace(IsClosed=False, Dispose=Mock()) for _ in range(2)]
            with patch.object(self.worker, "_nurbs_surface_definition",
                              return_value={"domain_u": [0, 1], "domain_v": [0, 1]},
                              side_effect=ValueError("record failed") if failure == "record" else None):
                if failure:
                    with self.assertRaises(ValueError):
                        self.worker._loft_command({}, 2, sources)
                else:
                    value, _ = self.worker._loft_command({}, 2, sources)
                    self.assertTrue(value["succeeded"])
                    self.assertEqual(value["originals_selected"], [True, True])
                    self.assertEqual(len(attributes), 6)
            self.assertEqual(set(objects), {0})
            for attribute in attributes:
                attribute.Dispose.assert_called_once_with()
            for source in sources:
                source.Dispose.assert_not_called()  # The _loft caller owns inputs.

    def test_loft_disposes_all_owned_inputs_and_results_on_success_and_failure(self):
        for failure in [None, "source", "build", "validity", "record"]:
            sources = []
            results = []
            def source(definition):
                if failure == "source" and sources:
                    raise ValueError("source failed")
                curve = SimpleNamespace(Dispose=Mock())
                sources.append(curve)
                return curve
            def build(*args):
                if failure == "build" and results:
                    raise ValueError("build failed")
                brep = SimpleNamespace(Faces=[object()], IsValid=failure != "validity", Dispose=Mock())
                results.append(brep)
                return [brep]
            self.worker.Rhino.Geometry = SimpleNamespace(
                LoftType=SimpleNamespace(Normal=0, Loose=1, Tight=2, Straight=3, Uniform=4),
                Point3d=SimpleNamespace(Unset=None), Brep=SimpleNamespace(CreateFromLoft=build))
            with patch.object(self.worker, "_nurbs_curve_from_definition", side_effect=source), patch.object(
                self.worker, "_nurbs_surface_definition", side_effect=ValueError("record failed") if failure == "record" else None,
                return_value={"surface": True}
            ):
                if failure:
                    with self.assertRaises(ValueError):
                        self.worker._loft({"curves": [{}, {}]}, 2)
                else:
                    value, _ = self.worker._loft({"curves": [{}, {}]}, 2)
                    self.assertEqual(value, [{"surface": True}])
                    self.assertEqual(len(results), 3)
            for item in sources + results:
                item.Dispose.assert_called_once_with()

    def test_brep_interchange_disposes_repeated_models_and_failed_recording(self):
        for fail in [False, True]:
            models = []
            class Brep:
                IsValid = True
            def read(path):
                self.assertEqual(path, "Z:\\owned\\brep.3dm")
                model = SimpleNamespace(Objects=[SimpleNamespace(Geometry=Brep())], Dispose=Mock())
                models.append(model)
                return model
            self.worker.Rhino.Geometry = SimpleNamespace(Brep=Brep)
            self.worker.Rhino.FileIO = SimpleNamespace(File3dm=SimpleNamespace(Read=read))
            with patch.object(self.worker, "_interchange_brep_record", return_value={"faces": []}), patch.object(
                self.worker, "_interchange_brep_mesh_flags", side_effect=ValueError("mesh failed") if fail else None,
                return_value={"closed": True}
            ):
                if fail:
                    with self.assertRaisesRegex(ValueError, "mesh failed"):
                        self.worker._three_dm_brep_interchange({"artifact_path": "/owned/brep.3dm"}, 2)
                else:
                    value, _ = self.worker._three_dm_brep_interchange({"artifact_path": "/owned/brep.3dm"}, 2)
                    self.assertEqual(value, {"faces": [], "mesh": {"closed": True}})
            self.assertEqual(len(models), 3)
            for model in models:
                model.Dispose.assert_called_once_with()

    def test_brep_interchange_disposes_invalid_import_without_repairing_it(self):
        class Brep:
            IsValid = False
            IsValidWithLog = Mock(return_value=(False, "invalid trim"))
        model = SimpleNamespace(Objects=[SimpleNamespace(Geometry=Brep())], Dispose=Mock())
        self.worker.Rhino.Geometry = SimpleNamespace(Brep=Brep)
        self.worker.Rhino.FileIO = SimpleNamespace(File3dm=SimpleNamespace(Read=lambda path: model))
        with self.assertRaisesRegex(ValueError, "invalid trim"):
            self.worker._three_dm_brep_interchange({"artifact_path": "/owned/brep.3dm"}, 2)
        model.Dispose.assert_called_once_with()

    def test_brep_interchange_mesh_probe_disposes_partial_results(self):
        for fail in [False, True]:
            parts = [SimpleNamespace(Dispose=Mock()), SimpleNamespace(Dispose=Mock())]
            mesh = SimpleNamespace(IsValid=True, IsClosed=True, Dispose=Mock(),
                                   GetNakedEdges=lambda: [],
                                   Append=Mock(side_effect=ValueError("append failed") if fail else None))
            parameters = SimpleNamespace(Dispose=Mock())
            def create_mesh():
                return mesh
            create_mesh.CreateFromBrep = lambda source, params: parts
            self.worker.Rhino.Geometry = SimpleNamespace(Mesh=create_mesh, MeshingParameters=lambda density: parameters)
            with patch.object(self.worker, "_coordinate_welded_mesh_flags", return_value=(True, True, False)):
                if fail:
                    with self.assertRaisesRegex(ValueError, "append failed"):
                        self.worker._interchange_brep_mesh_flags(object())
                else:
                    self.assertEqual(self.worker._interchange_brep_mesh_flags(object()),
                                     {"closed": True, "manifold": True, "oriented": True,
                                      "boundary_loops": 0, "boundaries_closed": True})
            for item in parts + [mesh, parameters]:
                item.Dispose.assert_called_once_with()

    def setUp(self):
        self.document = SimpleNamespace(
            ModelAbsoluteTolerance=0.01,
            ModelRelativeTolerance=0.01,
            ModelAngleToleranceRadians=0.02,
        )
        self.original = vars(self.document).copy()
        rhino = SimpleNamespace(
            RhinoDoc=SimpleNamespace(ActiveDoc=self.document),
            RhinoApp=SimpleNamespace(Version="test"),
        )
        spec = importlib.util.spec_from_file_location(
            "oracle_worker_test", Path(__file__).with_name("rhino_worker.py")
        )
        self.worker = importlib.util.module_from_spec(spec)
        with patch.dict("sys.modules", {"Rhino": rhino, "System": SimpleNamespace()}):
            spec.loader.exec_module(self.worker)
        self.request = {
            "protocol_version": 1,
            "operations": [{"id": "command", "op": "stub"}],
            "tolerance": {"absolute": 1e-8, "relative": 1e-10, "angular": 1e-6},
        }

    def test_applies_fixture_tolerance_during_commands_and_restores_document(self):
        def execute(operation, iterations, tolerance):
            self.assertEqual(self.document.ModelAbsoluteTolerance, 1e-8)
            self.assertEqual(self.document.ModelRelativeTolerance, 1e-10)
            self.assertEqual(self.document.ModelAngleToleranceRadians, 1e-6)
            return {"succeeded": True}, 100

        with patch.object(self.worker, "_execute", side_effect=execute), patch.object(
            self.worker, "_record_progress"
        ):
            response = self.worker._response(self.request)
        self.assertNotIn("error", response)
        self.assertEqual(response["results"][0]["value"], {"succeeded": True})
        self.assertEqual(vars(self.document), self.original)

    def test_failed_command_restores_document_and_discards_partial_results(self):
        self.request["operations"].append({"id": "failing", "op": "stub"})
        with patch.object(
            self.worker, "_execute", side_effect=[({"succeeded": True}, 100), ValueError("failed")]
        ), patch.object(self.worker, "_record_progress"):
            response = self.worker._response(self.request)
        self.assertIn("failed", response["error"])
        self.assertEqual(response["results"], [])
        self.assertEqual(vars(self.document), self.original)

    def test_rejected_tolerance_restores_settings_before_any_command_runs(self):
        class RejectingDocument(SimpleNamespace):
            def __setattr__(self, name, value):
                if name == "ModelRelativeTolerance" and value != 0.01:
                    return
                super().__setattr__(name, value)

        document = RejectingDocument(**self.original)
        self.worker.Rhino.RhinoDoc.ActiveDoc = document
        with patch.object(self.worker, "_execute") as execute:
            response = self.worker._response(self.request)
        execute.assert_not_called()
        self.assertIn("did not accept", response["error"])
        self.assertEqual(vars(document), self.original)

    def test_differential_only_records_do_not_invoke_length_solvers(self):
        point = SimpleNamespace(X=0.0, Y=0.0, Z=0.0)
        tangent = SimpleNamespace(X=1.0, Y=0.0, Z=0.0)
        curve = SimpleNamespace(
            Domain=SimpleNamespace(T0=0.0, T1=1.0, ParameterAt=lambda t: t),
            IsClosed=False,
            PointAt=lambda t: point,
            DerivativeAt=lambda t, order: [point, tangent, point],
            TangentAt=lambda t: tangent,
            GetLength=Mock(return_value=1.0),
            NormalizedLengthParameter=Mock(side_effect=lambda t, tolerance: (True, t)),
        )
        with patch.object(self.worker, "_nurbs_curve_definition", return_value={}):
            differential = self.worker._curve_native_record(curve, {"relative": 1e-12}, True)
            self.assertEqual(len(differential["samples"]), 33)
            self.assertNotIn("length", differential)
            self.assertNotIn("divisions", differential)
            curve.GetLength.assert_not_called()
            curve.NormalizedLengthParameter.assert_not_called()

            complete = self.worker._curve_native_record(curve, {"relative": 1e-12})
            self.assertEqual(complete["length"], 1.0)
            self.assertEqual(len(complete["divisions"]), 18)
            curve.GetLength.assert_called_once_with(1e-12)
            self.assertEqual(curve.NormalizedLengthParameter.call_count, 16)

    def test_sided_records_use_derivative_points_and_restrict_stationary_tangents(self):
        class Vector:
            def __init__(self, source):
                self.X, self.Y, self.Z = source.X, source.Y, source.Z

            def Unitize(self):
                length = math.hypot(self.X, self.Y, self.Z)
                if length == 0:
                    return False
                self.X, self.Y, self.Z = self.X / length, self.Y / length, self.Z / length
                return True

        def xyz(x, y=0.0, z=0.0):
            return SimpleNamespace(X=x, Y=y, Z=z)

        self.worker.Rhino.Geometry = SimpleNamespace(
            CurveEvaluationSide=SimpleNamespace(Below=-1, Above=1), Vector3d=Vector,
            Interval=lambda a, b: SimpleNamespace(T0=a, T1=b),
        )
        for stationary in [False, True]:
            with self.subTest(stationary=stationary):
                pieces = []

                def trim(interval):
                    piece = SimpleNamespace(
                        TangentAt=Mock(return_value=Vector(xyz(-1.0 if interval.T1 == 1.0 else 1.0))),
                        Dispose=Mock(),
                    )
                    pieces.append(piece)
                    return piece

                curve = SimpleNamespace(
                    Domain=SimpleNamespace(T0=0.0, T1=2.0, ParameterAt=lambda t: 2.0 * t),
                    IsClosed=False, PointAt=lambda t: xyz(42.0), TangentAt=lambda t: xyz(1.0),
                    DerivativeAt=lambda t, order, side=0: [xyz(10.0 + side), xyz(0.0 if stationary else 1.0), xyz(0.0)],
                    Trim=Mock(side_effect=trim),
                )
                with patch.object(self.worker, "_nurbs_curve_definition", return_value={}):
                    value = self.worker._curve_native_record(curve, {"relative": 1e-12}, True, [1.0])
                self.assertEqual(value["sides"][0]["left"]["point"], [9.0, 0.0, 0.0])
                self.assertEqual(value["sides"][0]["right"]["point"], [11.0, 0.0, 0.0])
                if stationary:
                    intervals = [call.args[0] for call in curve.Trim.call_args_list]
                    self.assertEqual([(i.T0, i.T1) for i in intervals], [(0.0, 1.0), (1.0, 2.0)])
                    self.assertEqual(value["sides"][0]["left"]["tangent"], [-1.0, 0.0, 0.0])
                    self.assertEqual(value["sides"][0]["right"]["tangent"], [1.0, 0.0, 0.0])
                    for piece in pieces:
                        piece.TangentAt.assert_called_once_with(1.0)
                        piece.Dispose.assert_called_once_with()
                else:
                    curve.Trim.assert_not_called()

    def test_surface_jets_prepare_exact_quadrants_before_timing_and_keep_derivative_order(self):
        class Interval:
            def __init__(self, a, b):
                self.T0, self.T1 = a, b

        self.worker.Rhino.Geometry = SimpleNamespace(Interval=Interval)
        vector = lambda x: SimpleNamespace(X=float(x), Y=0.0, Z=0.0)
        domain = Interval(0.0, 2.0)
        pieces = []

        def trim(u, v):
            piece = SimpleNamespace(
                Evaluate=Mock(return_value=(True, vector(len(pieces)), [vector(i) for i in range(1, 6)])),
                Dispose=Mock(),
            )
            pieces.append(piece)
            return piece

        surface = SimpleNamespace(
            Domain=lambda axis: domain, Trim=Mock(side_effect=trim), Dispose=Mock(),
            Evaluate=Mock(return_value=(True, vector(99), [vector(i) for i in range(1, 6)])),
        )
        samples = [{"parameter": [1.0, 1.0], "side_u": a, "side_v": b}
                   for a in ("left", "right") for b in ("left", "right")]
        samples += [{"parameter": uv} for uv in ([0.0, 0.0], [2.0, 2.0], [0.5, 0.5])]

        def measure(iterations, compute):
            self.assertEqual(len(pieces), 4)
            return compute(), 123

        with patch.object(self.worker, "_nurbs_surface_from_definition", return_value=surface), patch.object(
            self.worker, "_measure", side_effect=measure
        ):
            value, elapsed = self.worker._surface_jets({"surface": {}, "samples": samples}, 3)
        self.assertEqual(elapsed, 123)
        intervals = [tuple((i.T0, i.T1) for i in call.args) for call in surface.Trim.call_args_list]
        self.assertEqual(intervals, [((0.0, 1.0), (0.0, 1.0)), ((0.0, 1.0), (1.0, 2.0)),
                                     ((1.0, 2.0), (0.0, 1.0)), ((1.0, 2.0), (1.0, 2.0))])
        for index, record in enumerate(value["samples"]):
            self.assertEqual(record["point"][0], float(index if index < 4 else 99))
            for derivative, key in enumerate(["du", "dv", "duu", "duv", "dvv"], 1):
                self.assertEqual(record[key], [float(derivative), 0.0, 0.0])
        for piece in pieces:
            piece.Evaluate.assert_called_once_with(1.0, 1.0, 2)
            piece.Dispose.assert_called_once_with()
        surface.Dispose.assert_called_once_with()
        self.assertEqual(surface.Evaluate.call_count, 3)

    def test_surface_jet_evaluation_failure_disposes_the_prepared_quadrant(self):
        interval = lambda a, b: SimpleNamespace(T0=a, T1=b)
        self.worker.Rhino.Geometry = SimpleNamespace(Interval=interval)
        piece = SimpleNamespace(Evaluate=Mock(return_value=(False, None, None)), Dispose=Mock())
        surface = SimpleNamespace(Domain=lambda axis: interval(0.0, 2.0),
                                  Trim=Mock(return_value=piece), Dispose=Mock())
        with patch.object(self.worker, "_nurbs_surface_from_definition", return_value=surface):
            with self.assertRaisesRegex(ValueError, "second surface partials"):
                self.worker._surface_jets({"surface": {}, "samples": [
                    {"parameter": [1.0, 1.0], "side_u": "left"}
                ]}, 1)
        piece.Dispose.assert_called_once_with()
        surface.Dispose.assert_called_once_with()

    def test_curve_morph_probe_keeps_point_map_separate_and_disposes_all_fits(self):
        def xyz(x):
            return SimpleNamespace(X=x, Y=0.0, Z=0.0)

        candidates = []
        domain = SimpleNamespace(T0=0.0, T1=1.0, ParameterAt=lambda s: s)

        def duplicate():
            candidate = SimpleNamespace(Domain=domain, IsValid=True, Dispose=Mock(), PointAt=lambda t: xyz(t + 0.25))
            candidates.append(candidate)
            return candidate

        source = SimpleNamespace(Domain=domain, PointAt=xyz, DuplicateCurve=duplicate, Dispose=Mock())
        surface = SimpleNamespace(Dispose=Mock())
        morph = SimpleNamespace(MorphPoint=lambda p: xyz(p.X + 1.0), Morph=Mock(return_value=True), Dispose=Mock())
        self.worker.Rhino.Geometry = SimpleNamespace(
            Plane=lambda *args: SimpleNamespace(IsValid=True), Point2d=lambda *args: args,
            Morphs=SimpleNamespace(SplopSpaceMorph=lambda *args: morph),
        )
        operation = {"curve": {}, "surface": {}, "source_origin": [0.0] * 3,
                     "source_x": [1.0, 0.0, 0.0], "source_y": [0.0, 1.0, 0.0],
                     "uv": [0.3, 0.4], "scale": 1.0, "angle": 0.0}
        with patch.object(self.worker, "_nurbs_curve_from_definition", return_value=source), patch.object(
            self.worker, "_nurbs_surface_from_definition", return_value=surface
        ), patch.object(self.worker, "_point", side_effect=lambda x: x), patch.object(
            self.worker, "_vector", side_effect=lambda x: x
        ):
            value, _ = self.worker._curve_surface_morph(operation, 2, {"absolute": 1e-9})
        self.assertFalse(morph.QuickPreview)
        self.assertFalse(morph.PreserveStructure)
        self.assertEqual(morph.Tolerance, 1e-9)
        self.assertEqual(len(candidates), 3)
        self.assertEqual(len(value["exact_samples"]), 257)
        self.assertEqual(value["exact_samples"][0], [1.0, 0.0, 0.0])
        self.assertEqual(value["fitted_samples"][0], [0.25, 0.0, 0.0])
        for item in candidates + [source, surface, morph]:
            item.Dispose.assert_called_once_with()

    def test_brep_morph_probe_separates_samples_and_disposes_repeated_fits(self):
        def xyz(x):
            return SimpleNamespace(X=x, Y=0.0, Z=0.0)
        candidates = []
        def duplicate():
            candidate = SimpleNamespace(IsValid=True, Dispose=Mock())
            candidates.append(candidate)
            return candidate
        source = SimpleNamespace(DuplicateBrep=duplicate, Dispose=Mock())
        surface = SimpleNamespace(Dispose=Mock())
        morph = SimpleNamespace(MorphPoint=lambda p: xyz(p.X + 1.0),
                                Morph=Mock(return_value=True), Dispose=Mock())
        with patch.object(self.worker, "_trimmed_brep_from_definition", return_value=source), patch.object(
            self.worker, "_nurbs_surface_from_definition", return_value=surface
        ), patch.object(self.worker, "_surface_point_morph", return_value=morph), patch.object(
            self.worker, "_brep_morph_plan", return_value=[("vertex", 0)]
        ), patch.object(self.worker, "_brep_morph_point", side_effect=lambda g, s: xyz(0.0 if g is source else 0.25)), patch.object(
            self.worker, "_brep_morph_topology", return_value={"vertices": 1}
        ):
            value, _ = self.worker._brep_surface_morph({"source": {}, "surface": {}}, 2, {"absolute": 1e-9})
        self.assertEqual(value["exact_samples"], [[1.0, 0.0, 0.0]])
        self.assertEqual(value["fitted_samples"], [[0.25, 0.0, 0.0]])
        self.assertEqual(len(candidates), 3)
        for item in candidates + [source, surface, morph]:
            item.Dispose.assert_called_once_with()

    def test_trimmed_brep_builder_disposes_partial_brep_on_boundary_failure(self):
        brep = SimpleNamespace(Dispose=Mock())
        self.worker.Rhino.Geometry = SimpleNamespace(Brep=lambda: brep)
        with patch.object(self.worker, "_nurbs_curve_from_definition", side_effect=ValueError("curve failed")):
            with self.assertRaisesRegex(ValueError, "curve failed"):
                self.worker._trimmed_brep_from_definition({"boundaries": [{"curve": {}}]}, {"absolute": 1e-9})
        brep.Dispose.assert_called_once_with()

    def test_brep_morph_edge_comparison_uses_closest_point_not_source_parameter(self):
        point = object()
        edge = SimpleNamespace(ClosestPoint=Mock(return_value=(True, 0.75)),
                               PointAt=Mock(return_value=point))
        with patch.object(self.worker, "_point", return_value=point):
            actual = self.worker._brep_morph_corresponding_point(
                SimpleNamespace(Edges=[edge]), ("edge", 0, 0.25), [1.0, 2.0, 3.0])
        self.assertIs(actual, point)
        edge.ClosestPoint.assert_called_once_with(point)
        edge.PointAt.assert_called_once_with(0.75)

    def test_brep_meshing_disposes_parts_parameters_and_repeated_combined_meshes(self):
        source = SimpleNamespace(Dispose=Mock())
        parameters = SimpleNamespace(Dispose=Mock())
        parts, combined = [], []
        def create_parts(_source, _parameters):
            part = SimpleNamespace(Dispose=Mock())
            parts.append(part)
            return [part]
        def create_combined():
            mesh = SimpleNamespace(Append=Mock(), IsValid=True, Dispose=Mock())
            combined.append(mesh)
            return mesh
        create_combined.CreateFromBrep = create_parts
        self.worker.Rhino.Geometry = SimpleNamespace(Mesh=create_combined, MeshingParameters=lambda density: parameters)
        with patch.object(self.worker, "_refined_box_brep", return_value=source), patch.object(
            self.worker, "_brep_mesh_boundary_record", return_value={"boundary_loops": 1}
        ):
            value, _ = self.worker._brep_mesh_boundaries({"density": 0.0, "simple_planes": False}, 2, {"absolute": 1e-9})
        self.assertEqual(value, {"boundary_loops": 1})
        self.assertFalse(parameters.JaggedSeams)
        self.assertEqual(len(combined), 3)
        for item in parts + combined + [source, parameters]:
            item.Dispose.assert_called_once_with()

    def test_brep_meshing_disposes_partial_combined_mesh_after_append_failure(self):
        source = SimpleNamespace(Dispose=Mock())
        parameters = SimpleNamespace(Dispose=Mock())
        part = SimpleNamespace(Dispose=Mock())
        mesh = SimpleNamespace(Append=Mock(side_effect=ValueError("append failed")), Dispose=Mock())
        def create_combined():
            return mesh
        create_combined.CreateFromBrep = lambda source, parameters: [part]
        self.worker.Rhino.Geometry = SimpleNamespace(Mesh=create_combined, MeshingParameters=lambda density: parameters)
        with patch.object(self.worker, "_refined_box_brep", return_value=source):
            with self.assertRaisesRegex(ValueError, "append failed"):
                self.worker._brep_mesh_boundaries({"density": 0.0, "simple_planes": False}, 1, {"absolute": 1e-9})
        for item in [source, parameters, part, mesh]:
            item.Dispose.assert_called_once_with()

    def test_coordinate_topology_probe_rejects_geometry_changes_and_disposes_duplicate(self):
        for changed in [False, True]:
            welded = SimpleNamespace(Vertices=SimpleNamespace(CombineIdentical=Mock()),
                                     IsManifold=Mock(return_value=(True, True, False)), Dispose=Mock())
            source = SimpleNamespace(DuplicateMesh=lambda: welded)
            with patch.object(self.worker, "_mesh_polygon_positions", side_effect=[[[1.0]], [[2.0 if changed else 1.0]]]):
                if changed:
                    with self.assertRaisesRegex(ValueError, "changed mesh polygon geometry"):
                        self.worker._coordinate_welded_mesh_flags(source)
                else:
                    self.assertEqual(self.worker._coordinate_welded_mesh_flags(source), (True, True, False))
            welded.Vertices.CombineIdentical.assert_called_once_with(True, True)
            welded.Dispose.assert_called_once_with()

    def test_brep_morph_releases_failed_candidate_and_all_owned_geometry(self):
        candidate = SimpleNamespace(Dispose=Mock())
        source = SimpleNamespace(DuplicateBrep=lambda: candidate, Dispose=Mock())
        surface = SimpleNamespace(Dispose=Mock())
        morph = SimpleNamespace(Morph=Mock(return_value=False), Dispose=Mock())
        with patch.object(self.worker, "_trimmed_brep_from_definition", return_value=source), patch.object(
            self.worker, "_nurbs_surface_from_definition", return_value=surface
        ), patch.object(self.worker, "_surface_point_morph", return_value=morph), patch.object(
            self.worker, "_brep_morph_plan", return_value=[]
        ):
            with self.assertRaisesRegex(ValueError, "could not morph"):
                self.worker._brep_surface_morph({"source": {}, "surface": {}}, 1, {"absolute": 1e-9})
        for item in [candidate, source, surface, morph]:
            item.Dispose.assert_called_once_with()

    def test_curve_morph_probe_releases_source_when_surface_creation_fails(self):
        source = SimpleNamespace(Dispose=Mock())
        with patch.object(self.worker, "_nurbs_curve_from_definition", return_value=source), patch.object(
            self.worker, "_nurbs_surface_from_definition", side_effect=ValueError("surface failed")
        ):
            with self.assertRaisesRegex(ValueError, "surface failed"):
                self.worker._curve_surface_morph({"curve": {}, "surface": {}}, 1, {"absolute": 1e-9})
        source.Dispose.assert_called_once_with()

    def test_surface_morph_probe_uses_native_uv_and_disposes_successful_and_failed_fits(self):
        def xyz(x, y=0.0):
            return SimpleNamespace(X=x, Y=y, Z=0.0)

        domains = [SimpleNamespace(T0=-7.0, T1=13.0, ParameterAt=lambda s: -7.0 + 20.0 * s),
                   SimpleNamespace(T0=2.0, T1=6.0, ParameterAt=lambda s: 2.0 + 4.0 * s)]
        candidates = []

        def duplicate():
            face = SimpleNamespace(Domain=lambda axis: domains[axis],
                                   PointAt=lambda u, v: xyz(u + 0.25, v))
            class Faces(list):
                Count = 1
            candidate = SimpleNamespace(Faces=Faces([face]), IsValid=True, Dispose=Mock())
            candidates.append(candidate)
            return candidate

        source = SimpleNamespace(Domain=lambda axis: domains[axis], PointAt=xyz,
                                 ToBrep=duplicate, Dispose=Mock())
        surface = SimpleNamespace(Dispose=Mock())
        morph = SimpleNamespace(MorphPoint=lambda p: xyz(p.X + 1.0, p.Y),
                                Morph=Mock(return_value=True), Dispose=Mock())
        self.worker.Rhino.Geometry = SimpleNamespace(
            Plane=lambda *args: SimpleNamespace(IsValid=True), Point2d=lambda *args: args,
            Morphs=SimpleNamespace(SplopSpaceMorph=lambda *args: morph))
        operation = {"source": {}, "surface": {}, "source_origin": [0.0] * 3,
                     "source_x": [1.0, 0.0, 0.0], "source_y": [0.0, 1.0, 0.0],
                     "uv": [0.3, 0.4], "scale": 1.0, "angle": 0.0, "fit_tolerance": 1e-7}
        with patch.object(self.worker, "_nurbs_surface_from_definition", side_effect=[source, surface]), patch.object(
            self.worker, "_point", side_effect=lambda x: x
        ), patch.object(self.worker, "_vector", side_effect=lambda x: x):
            value, _ = self.worker._surface_surface_morph(operation, 2, {"absolute": 1e-9})
        self.assertFalse(morph.QuickPreview)
        self.assertFalse(morph.PreserveStructure)
        self.assertEqual(morph.Tolerance, 1e-7)
        self.assertEqual(value["domain_u"], [-7.0, 13.0])
        self.assertEqual(value["domain_v"], [2.0, 6.0])
        self.assertEqual(len(value["exact_samples"]), 1089)
        self.assertEqual(value["exact_samples"][0], [-6.0, 2.0, 0.0])
        self.assertEqual(value["fitted_samples"][0], [-6.75, 2.0, 0.0])
        self.assertEqual(value["exact_samples"][-1], [14.0, 6.0, 0.0])
        self.assertEqual(len(candidates), 3)
        for item in candidates + [source, surface, morph]:
            item.Dispose.assert_called_once_with()
            item.Dispose.reset_mock()

        morph.Morph.return_value = False
        with patch.object(self.worker, "_nurbs_surface_from_definition", side_effect=[source, surface]), patch.object(
            self.worker, "_point", side_effect=lambda x: x
        ), patch.object(self.worker, "_vector", side_effect=lambda x: x):
            with self.assertRaisesRegex(ValueError, "could not morph"):
                self.worker._surface_surface_morph(operation, 1, {"absolute": 1e-9})
        for item in [candidates[-1], source, surface, morph]:
            item.Dispose.assert_called_once_with()


if __name__ == "__main__":
    unittest.main()

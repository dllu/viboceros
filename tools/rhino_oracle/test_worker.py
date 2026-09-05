"""Test the worker's host-independent orchestration with a simulated document."""

import importlib.util
import math
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch


class RhinoWorkerTests(unittest.TestCase):
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

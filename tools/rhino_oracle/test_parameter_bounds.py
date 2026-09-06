"""Independent trim-image references and ownership of public Rhino objects."""
from math import sqrt
from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import Mock, patch
from . import test_worker


class Point:
    def __init__(self, x, y, z): self.X, self.Y, self.Z = x, y, z
    def DistanceTo(self, p): return sqrt((self.X-p.X)**2+(self.Y-p.Y)**2+(self.Z-p.Z)**2)


class Box:
    def __init__(self, low, high):
        self.Min, self.Max, self.IsValid = Point(*low), Point(*high), True
    def Union(self, b):
        self.Min = Point(*(min(getattr(self.Min,k),getattr(b.Min,k)) for k in ["X","Y","Z"]))
        self.Max = Point(*(max(getattr(self.Max,k),getattr(b.Max,k)) for k in ["X","Y","Z"]))


class ParameterBoundsWorkerTests(TestCase):
    def setUp(self):
        test_worker.RhinoWorkerTests.setUp(self)
        self.tolerance = dict(absolute=1e-9, relative=1e-12, angular=1e-10)

    def inputs(self):
        domain = SimpleNamespace(T0=0, T1=1, ParameterAt=lambda t:t)
        surface = SimpleNamespace(PointAt=Mock(side_effect=lambda u,v:Point(u,v,u*u)), Dispose=Mock())
        curve = SimpleNamespace(Domain=domain, PointAt=Mock(side_effect=lambda t:Point(t,0,0)), Dispose=Mock())
        reference = SimpleNamespace(Domain=domain, PointAt=Mock(side_effect=lambda t:Point(t,0,t*t)),
                                    GetBoundingBox=Mock(return_value=Box([0,0,0],[1,0,1])), Dispose=Mock())
        operation = dict(op="surface_parameter_curve_bounds",surface={},reference_curve={},
                         parameter_curve={"control_points":[{"point":[0,0,0]}]})
        return operation, surface, curve, reference

    def test_parameter_image_uses_independent_bounds_and_disposes_every_owned_input(self):
        for failure in [None,"sample","bounds","domain"]:
            with self.subTest(failure=failure):
                operation, surface, curve, reference = self.inputs()
                if failure == "sample": reference.PointAt.side_effect = lambda t:Point(t,0,t*t+0.1)
                if failure == "bounds": reference.GetBoundingBox.return_value.IsValid = False
                if failure == "domain": reference.Domain = SimpleNamespace(T0=0,T1=2)
                with patch.object(self.worker,"_nurbs_surface_from_definition",return_value=surface), \
                     patch.object(self.worker,"_nurbs_curve_from_definition",side_effect=[curve,reference]):
                    if failure:
                        with self.assertRaises(ValueError): self.worker._execute(operation,99,self.tolerance)
                    else:
                        value,elapsed = self.worker._execute(operation,99,self.tolerance)
                        self.assertEqual(elapsed,0)
                        self.assertEqual(value["min"],[0,0,0])
                        self.assertEqual(value["max"],[1,0,1])
                        self.assertEqual(len(value["samples"]),65)
                        self.assertEqual(value["reference_samples"],value["samples"])
                        self.assertEqual(value["samples"][32],[0.5,0,0.25])
                        reference.GetBoundingBox.assert_called_once_with(True)
                for geometry in [surface,curve,reference]: geometry.Dispose.assert_called_once_with()

    def test_invalid_or_partially_constructed_parameter_inputs_do_not_leak(self):
        operation, surface, curve, reference = self.inputs()
        with patch.object(self.worker,"_nurbs_surface_from_definition",return_value=surface), \
             patch.object(self.worker,"_nurbs_curve_from_definition",side_effect=[curve,ValueError("construction")]):
            with self.assertRaisesRegex(ValueError,"construction"):
                self.worker._execute(operation,1,self.tolerance)
        surface.Dispose.assert_called_once_with()
        curve.Dispose.assert_called_once_with()
        reference.Dispose.assert_not_called()
        operation["parameter_curve"]["control_points"][0]["point"][2] = 1
        with patch.object(self.worker,"_nurbs_surface_from_definition") as create:
            with self.assertRaisesRegex(ValueError,"zero Z"):
                self.worker._execute(operation,1,self.tolerance)
            create.assert_not_called()

    def test_trim_boundary_references_union_every_edge_and_only_dispose_the_owned_brep(self):
        for missing_edge in [False,True]:
            domain=SimpleNamespace(ParameterAt=lambda t:t)
            edges=[SimpleNamespace(GetBoundingBox=Mock(return_value=Box([-2,0,0],[0,1,0])),Dispose=Mock()),
                   SimpleNamespace(GetBoundingBox=Mock(return_value=Box([0,-1,0],[2,0,0])),Dispose=Mock())]
            trims=[SimpleNamespace(Edge=edge,Domain=domain,PointAt=lambda t:Point(t,0,0)) for edge in edges]
            if missing_edge: trims[1].Edge=None
            face=SimpleNamespace(Loops=[SimpleNamespace(Trims=[trim]) for trim in trims],PointAt=lambda u,v:Point(u,v,0))
            brep=SimpleNamespace(Faces=[face],Dispose=Mock())
            with patch.object(self.worker,"_trimmed_brep_from_definition",return_value=brep):
                if missing_edge:
                    with self.assertRaisesRegex(ValueError,"explicit spatial edges"):
                        self.worker._execute(dict(op="trim_boundary_bounds"),1,self.tolerance)
                else:
                    value,elapsed=self.worker._execute(dict(op="trim_boundary_bounds"),1,self.tolerance)
                    self.assertEqual(elapsed,0)
                    self.assertEqual(value["faces"][0]["min"],[-2,-1,0])
                    self.assertEqual(value["faces"][0]["max"],[2,1,0])
                    self.assertEqual([len(s) for s in value["faces"][0]["samples"]],[65,65])
            brep.Dispose.assert_called_once_with()
            for edge in edges: edge.Dispose.assert_not_called()

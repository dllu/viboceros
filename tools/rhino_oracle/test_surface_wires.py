"""Surface-wire probes own their inputs, B-reps and returned curve arrays."""
from types import SimpleNamespace as NS
from unittest import TestCase
from unittest.mock import Mock, patch
from . import test_worker


class SurfaceWireWorkerTests(TestCase):
    def setUp(self):
        test_worker.RhinoWorkerTests.setUp(self)
        self.source = NS(Dispose=Mock())
        self.curves = [NS(Dispose=Mock(), PointAt=Mock(side_effect=lambda t: NS(X=t,Y=0,Z=0))), NS(Dispose=Mock(), PointAt=lambda t: NS(X=1-t,Y=1,Z=0))]
        self.brep = NS(Dispose=Mock(), IsValidWithLog=Mock(return_value=(True,"")),
                       GetWireframe=Mock(return_value=self.curves), Vertices=NS(Count=4),
                       Edges=NS(Count=4), Faces=NS(Count=1), IsSolid=False,
                       Trims=[NS(TrimType="Boundary") for _ in range(4)])
        self.worker.Rhino.Geometry = NS(Brep=NS(CreateFromSurface=Mock(return_value=self.brep)),
                                       Interval=lambda a,b:NS(T0=a,T1=b), BrepTrimType=NS(Seam="Seam",Singular="Singular"))

    def execute(self):
        with patch.object(self.worker,"_nurbs_surface_from_definition",return_value=self.source), \
             patch.object(self.worker,"_measure",side_effect=lambda n,f:(f(),0)):
            return self.worker._execute(dict(op="surface_wires",surface={},density=3),1,{})[0]

    def test_surface_topology_wires_and_closed_orientation_sampling(self):
        # A closed curve with equal endpoints requires the interior samples to
        # choose its orientation; return a parabola with a closed x coordinate.
        self.curves[1].PointAt=lambda t:NS(X=0,Y=t*(1-t),Z=t*(1-t)*(1-2*t))
        value=self.execute()
        self.assertEqual(value["surface_wires"],value["brep_wires"])
        self.assertEqual((value["vertices"],value["edges"],value["faces"],value["trims"]),(4,4,1,4))
        self.assertEqual(value["seam_trims"],0)
        self.assertEqual(value["singular_trims"],0)
        closed=value["surface_wires"][0]
        self.assertEqual(closed[0],closed[-1])
        self.assertLess(closed[1][2],0)
        self.brep.GetWireframe.assert_called_once_with(3)
        for item in [self.source,self.brep]+self.curves:
            item.Dispose.assert_called_once_with()

    def test_sampling_failure_disposes_the_entire_returned_array(self):
        self.curves[0].PointAt.side_effect=RuntimeError("sample")
        with self.assertRaisesRegex(RuntimeError,"sample"):
            self.execute()
        for item in [self.source,self.brep]+self.curves:
            item.Dispose.assert_called_once_with()

    def test_ordering_quantization_does_not_round_geometry(self):
        x=1.0000000000000002
        self.curves[0].PointAt=lambda t:NS(X=1.0,Y=1.0,Z=t)
        self.curves[1].PointAt=lambda t:NS(X=x,Y=-1.0,Z=t)
        records=self.execute()["surface_wires"]
        self.assertEqual(records[0][0],[x,-1.0,0.0])
        self.assertEqual(records[1][0],[1.0,1.0,0.0])
        self.assertEqual(self.worker._surface_wire_sort_key([[1e308,-1e308,0.0]]),(1e308,-1e308,0.0))

    def test_creation_failure_still_disposes_source(self):
        self.worker.Rhino.Geometry.Brep.CreateFromSurface.return_value=None
        with self.assertRaisesRegex(ValueError,"construction"):
            self.execute()
        self.source.Dispose.assert_called_once_with()
        self.brep.Dispose.assert_not_called()

    def test_invalid_density_does_not_allocate_geometry(self):
        for density in [-2,100,True,1.5,"1"]:
            with patch.object(self.worker,"_nurbs_surface_from_definition") as build:
                with self.assertRaises(ValueError):
                    self.worker._execute(dict(op="surface_wires",surface={},density=density),1,{})
                build.assert_not_called()

"""SDK routing and ownership mocks; these do not assert licensed Rhino parity."""
import copy
import sys
from types import SimpleNamespace as NS
from unittest import TestCase, mock
from .chamfer_edge_probe import request, validate, run

class Arrays:
    def __getitem__(self, kind): return list

class ChamferTests(TestCase):
    def test_strict_input_fields(self):
        for op in request()['operations']: validate(op)
        for key, value in [('distance', True), ('distance', 0), ('distance', float('nan')),
                           ('rail_type', 'DistFromEdge'), ('edge_points', []),
                           ('edge_points', [[True, 0, 1]]), ('edge_points', [[0, 1]]),
                           ('source', dict(type='mesh')), ('extra', 1)]:
            op = copy.deepcopy(request()['operations'][0]); op[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): validate(op)

    def host(self):
        class Brep: pass
        source = Brep(); source.IsSolid = True; source.Dispose = mock.Mock()
        source.Edges = [NS(AdjacentFaces=lambda:[0,1], ClosestPoint=lambda p:(True, 0.5),
                           PointAt=lambda t:NS(DistanceTo=lambda p:0.))]
        output = mock.Mock(IsValid=True, IsSolid=True); output.Faces.Count = 7
        output.GetBoundingBox.return_value = NS(Min=[0,0,0], Max=[10,8,6])
        mass = mock.Mock(Volume=478.5)
        Brep.CreateFilletEdges = mock.Mock(return_value=[output])
        G = NS(Brep=Brep, Point3d=lambda *args:args, RailType=NS(DistanceBetweenRails=2, RollingBall=1),
               BlendType=NS(Chamfer=0), VolumeMassProperties=NS(Compute=mock.Mock(return_value=mass)))
        host = {'Rhino':NS(Geometry=G), '_object_source':lambda *args:source,
                '_measure':lambda n,f:(f(),0), '_xyz':lambda p:p}
        return host, source, output, mass

    @mock.patch.dict(sys.modules, {'System':NS(Array=Arrays(), Int32=int, Double=float)})
    def test_public_sdk_modes_and_owned_disposal(self):
        for name, rail in [('DistBetweenRails',2), ('RollingBall',1)]:
            host, source, output, mass = self.host()
            op = copy.deepcopy(request()['operations'][0]); op['rail_type'] = name
            value, _ = run(op, host, 1, request()['tolerance'])
            host['Rhino'].Geometry.Brep.CreateFilletEdges.assert_called_once_with(source, [0], [1.], [1.], 0, rail, False, 1e-6, 1e-8)
            self.assertEqual(value['breps'][0]['volume'], 478.5)
            for obj in (source, output, mass): obj.Dispose.assert_called_once()

    @mock.patch.dict(sys.modules, {'System':NS(Array=Arrays(), Int32=int, Double=float)})
    def test_ambiguous_edges_and_partial_failures_clean_up(self):
        for stage in ('ambiguous','null','invalid','mass'):
            host, source, output, mass = self.host(); G = host['Rhino'].Geometry
            if stage == 'ambiguous': source.Edges *= 2
            elif stage == 'null': G.Brep.CreateFilletEdges.return_value = None
            elif stage == 'invalid': output.IsValid = False
            else: G.VolumeMassProperties.Compute.return_value = None
            with self.subTest(stage=stage), self.assertRaises(ValueError):
                run(request()['operations'][0], host, 1, request()['tolerance'])
            source.Dispose.assert_called_once()
            if stage in ('invalid','mass'): output.Dispose.assert_called_once()

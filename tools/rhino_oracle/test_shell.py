"""SDK routing and ownership checks with mocks; no licensed parity claim."""
import copy
from types import SimpleNamespace as NS
from unittest import TestCase, mock
from .shell_probe import request, run, validate


class ShellTests(TestCase):
    def test_strict_fixtures(self):
        for op in request()['operations']: validate(op)
        for key, value in [('thickness', True), ('thickness', 0), ('thickness', float('nan')),
                           ('opening_points', []), ('opening_points', [[True, 1, 2]]),
                           ('opening_points', [[0, 1]]), ('extra', 1), ('source', dict(type='mesh'))]:
            op = copy.deepcopy(request()['operations'][0]); op[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError): validate(op)

    def host(self):
        class Brep: pass
        source = Brep(); source.IsSolid = True; source.Dispose = mock.Mock()
        class Faces(list):
            @property
            def Count(self): return len(self)
        source.Faces = Faces([NS(ClosestPoint=lambda p:(True, 0, 0),
                                 PointAt=lambda u,v:NS(DistanceTo=lambda p:0)),
                              NS(ClosestPoint=lambda p:(False, 0, 0))])
        output = mock.Mock(IsSolid=True, IsValid=True)
        output.Faces.Count = 10
        output.GetBoundingBox.return_value = NS(Min=[0,0,0], Max=[10,8,6])
        mass = mock.Mock(Volume=240.)
        Brep.CreateShell = mock.Mock(return_value=[output])
        G = NS(Brep=Brep, Point3d=lambda *xyz:xyz, VolumeMassProperties=NS(Compute=mock.Mock(return_value=mass)))
        host = {'Rhino':NS(Geometry=G), '_object_source':lambda *args:source,
                '_measure':lambda n,f:(f(),0), '_xyz':lambda p:p}
        return host, source, output, mass

    def test_signed_inward_distance_face_resolution_and_disposal(self):
        host, source, output, mass = self.host()
        value, _ = run(request()['operations'][0], host, 1, request()['tolerance'])
        host['Rhino'].Geometry.Brep.CreateShell.assert_called_once_with(source, [0], -1., 1e-6)
        self.assertEqual(value['breps'][0]['volume'], 240.)
        for item in (source, output, mass): item.Dispose.assert_called_once()

    def test_failure_cleanup_and_source_rejection(self):
        for stage in ('null', 'invalid', 'mass', 'ambiguous'):
            host, source, output, mass = self.host()
            G = host['Rhino'].Geometry
            if stage == 'null': G.Brep.CreateShell.return_value = None
            elif stage == 'invalid': output.IsValid = False
            elif stage == 'mass': G.VolumeMassProperties.Compute.return_value = None
            else: source.Faces[1] = source.Faces[0]
            with self.subTest(stage=stage), self.assertRaises(ValueError):
                run(request()['operations'][0], host, 1, request()['tolerance'])
            source.Dispose.assert_called_once()
            if stage in ('invalid', 'mass'): output.Dispose.assert_called_once()

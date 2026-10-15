"""Public SDK wiring and ownership; mock tests do not establish Rhino parity."""
import copy
import json
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase, mock
from . import test_surface_extrusion as straight
from .surface_apex_extrusion_probe import request, run, validate
ROOT = Path(__file__).resolve().parents[2]


class SurfaceApexExtrusionTests(TestCase):
    def test_independent_fixtures_and_strict_options(self):
        q = request()
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/surface_apex_extrusion.json').read_text()))
        for op in q['operations']: validate(op)
        for field, value in [('face', -1), ('face', True), ('solid', 1), ('apex', [1.,2.]),
                             ('apex', [1.,True,6.]), ('apex', [1.,float('inf'),6.]),
                             ('source', dict(type='mesh')), ('extra', 1)]:
            op = copy.deepcopy(q['operations'][0]); op[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError): validate(op)

    def host(self):
        host, source, brep, face, output, _, mass = straight.SurfaceExtrusionTests().host()
        G = host['Rhino'].Geometry
        cap = mock.Mock()
        curves, walls, parts = [mock.Mock() for _ in range(2)], [mock.Mock() for _ in range(2)], [mock.Mock() for _ in range(2)]
        cap.Edges = [NS(Valence=1, DuplicateCurve=mock.Mock(return_value=c)) for c in curves]
        face.DuplicateFace = mock.Mock(return_value=cap)
        for wall, part in zip(walls, parts): wall.ToBrep.return_value = part
        G.EdgeAdjacency = NS(Naked=1)
        G.Surface.CreateExtrusionToPoint = mock.Mock(side_effect=walls)
        G.Brep.JoinBreps = mock.Mock(return_value=[output])
        host['_point'] = lambda p:p
        return host, source, brep, face, output, cap, curves, walls, parts, mass

    def test_actual_source_cap_and_owned_boundary_curves_reach_public_sdk(self):
        host, source, brep, face, output, cap, curves, walls, parts, mass = self.host()
        op = request()['operations'][0]
        value, _ = run(op, host, 1, request()['tolerance'])
        face.DuplicateFace.assert_called_once_with(False)
        self.assertEqual(host['Rhino'].Geometry.Surface.CreateExtrusionToPoint.call_args_list,
                         [mock.call(c, op['apex']) for c in curves])
        host['Rhino'].Geometry.Brep.JoinBreps.assert_called_once_with([cap]+parts, 1e-6)
        self.assertEqual(value['breps'][0]['volume'], 12.)
        for owned in [source, brep, output, cap, mass]+curves+walls+parts: owned.Dispose.assert_called_once()

    def test_uncapped_parts_and_partial_failures_dispose_every_allocated_object(self):
        for stage in ('uncapped', 'curve', 'wall', 'join', 'invalid', 'mass'):
            with self.subTest(stage=stage):
                host, source, brep, face, output, cap, curves, walls, parts, mass = self.host()
                G = host['Rhino'].Geometry
                op = copy.deepcopy(request()['operations'][0])
                if stage == 'uncapped':
                    op['solid'] = False; output.IsSolid = False
                elif stage == 'curve': cap.Edges[0].DuplicateCurve.return_value = None
                elif stage == 'wall': G.Surface.CreateExtrusionToPoint.side_effect = [walls[0], None]
                elif stage == 'join': G.Brep.JoinBreps.return_value = None
                elif stage == 'invalid': output.IsValid = False
                else: G.VolumeMassProperties.Compute.return_value = None
                if stage == 'uncapped':
                    value, _ = run(op, host, 1, request()['tolerance'])
                    self.assertIsNone(value['breps'][0]['volume'])
                    G.Brep.JoinBreps.assert_called_once_with(parts, 1e-6)
                    mass.Dispose.assert_not_called()
                else:
                    with self.assertRaises(ValueError): run(op, host, 1, request()['tolerance'])
                for owned in (source, brep, cap): owned.Dispose.assert_called_once()
                if stage != 'curve':
                    curves[0].Dispose.assert_called_once(); walls[0].Dispose.assert_called_once(); parts[0].Dispose.assert_called_once()
                if stage in ('uncapped', 'invalid', 'mass'): output.Dispose.assert_called_once()

    def test_disjoint_uncapped_shells_append_and_aliased_results_dispose_once(self):
        host, source, brep, face, output, cap, curves, walls, parts, mass = self.host()
        G = host['Rhino'].Geometry
        combined = mock.Mock()
        combined.IsValid = True; combined.IsSolid = False; combined.Faces.Count = 2
        combined.GetBoundingBox.return_value = NS(Min=[-3.,-3.,0.], Max=[3.,3.,6.])
        G.Brep.__new__ = staticmethod(lambda cls: combined)
        G.Brep.JoinBreps.return_value = parts
        op = copy.deepcopy(request()['operations'][0]); op['solid'] = False
        value, _ = run(op, host, 1, request()['tolerance'])
        self.assertEqual(value['breps'][0]['face_count'], 2)
        self.assertEqual(combined.Append.call_args_list, [mock.call(p) for p in parts])
        for owned in [source, brep, cap, combined]+curves+walls+parts: owned.Dispose.assert_called_once()
        mass.Dispose.assert_not_called()

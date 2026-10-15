"""Surface taper public SDK routing and cleanup; no licensed parity assertion."""
import copy
import json
import math
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase, mock
from . import test_surface_extrusion as straight
from .surface_tapered_extrusion_probe import request, run, validate
ROOT = Path(__file__).resolve().parents[2]


class Vector(straight.Vector):
    @property
    def Length(self): return math.hypot(*self.values)
    def Unitize(self):
        length = self.Length
        if not length: return False
        self.values = tuple(x/length for x in self.values)
        return True


class SurfaceTaperedExtrusionTests(TestCase):
    def test_independent_fixtures_and_strict_inputs(self):
        q = request()
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/surface_tapered_extrusion.json').read_text()))
        for op in q['operations']: validate(op)
        for field, value in [('face', True), ('face', -1), ('solid', 1), ('round_corners', 'Round'),
                             ('displacement', [0.,0.,0.]), ('displacement', [True,0.,2.]),
                             ('displacement', [0.,float('nan'),2.]), ('draft_angle', 90.),
                             ('draft_angle', float('inf')), ('source', dict(type='mesh')), ('extra', 1)]:
            op = copy.deepcopy(q['operations'][0]); op[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError): validate(op)

    def host(self):
        host, source, brep, face, output, path, mass = straight.SurfaceExtrusionTests().host()
        G = host['Rhino'].Geometry
        cap, merged = mock.Mock(), mock.Mock()
        edges, profiles, walls = [mock.Mock() for _ in range(2)], [mock.Mock() for _ in range(2)], [mock.Mock() for _ in range(2)]
        trims = [NS(Edge=NS(DuplicateCurve=mock.Mock(return_value=c)), IsReversed=mock.Mock(return_value=i==1)) for i,c in enumerate(edges)]
        cap.Loops = [NS(LoopType=i+1, Trims=[trim]) for i,trim in enumerate(trims)]
        for i,profile in enumerate(profiles):
            profile.IsClosed = True; profile.PointAtStart = [3.-2.*i,0.,0.]
            profile.ClosedCurveOrientation.return_value = 1 if i==0 else -1
            profile.Reverse.return_value = True
        face.DuplicateFace = mock.Mock(return_value=cap)
        merged.CapPlanarHoles.return_value = output
        G.BrepLoopType = NS(Outer=1, Inner=2)
        G.Curve = NS(JoinCurves=mock.Mock(side_effect=[[p] for p in profiles]))
        G.CurveOrientation = NS(Clockwise=-1)
        G.Plane = mock.Mock(return_value='plane')
        G.ExtrudeCornerType = NS(Sharp=1, Round=2)
        G.Brep.CreateFromTaperedExtrude = mock.Mock(side_effect=[[w] for w in walls])
        G.Brep.JoinBreps = mock.Mock(return_value=[merged])
        host['_vector'] = Vector
        return host, source, brep, face, output, path, mass, cap, merged, edges, profiles, walls

    def test_hole_angle_orientation_base_cap_and_ownership(self):
        host, source, brep, face, output, path, mass, cap, merged, edges, profiles, walls = self.host()
        G = host['Rhino'].Geometry
        op = next(o for o in request()['operations'] if o['id'].endswith('ring_inner'))
        value, _ = run(op, host, 1, request()['tolerance'])
        calls = G.Brep.CreateFromTaperedExtrude.call_args_list
        self.assertEqual([c.args[0] for c in calls], profiles)
        self.assertEqual([c.args[1] for c in calls], [2.,2.])
        self.assertAlmostEqual(calls[0].args[4], math.radians(10.))
        self.assertAlmostEqual(calls[1].args[4], math.radians(-10.))
        self.assertEqual(calls[0].args[5:], (1, 1e-5, 1e-8))
        edges[1].Reverse.assert_called_once(); profiles[1].Reverse.assert_called_once()
        G.Brep.JoinBreps.assert_called_once_with([cap]+walls, 1e-5)
        merged.CapPlanarHoles.assert_called_once_with(1e-5)
        self.assertEqual(value['breps'][0]['volume'], 12.)
        for obj in [source,brep,cap,merged,output,mass]+edges+profiles+walls: obj.Dispose.assert_called_once()
        path.Dispose.assert_not_called()

    def test_zero_draft_uses_face_extrusion_and_keeps_displacement_length(self):
        host, source, brep, face, output, path, mass, cap, merged, edges, profiles, walls = self.host()
        op = copy.deepcopy(request()['operations'][0]); op['draft_angle'] = 0.
        run(op, host, 1, request()['tolerance'])
        face.CreateExtrusion.assert_called_once_with(path, True)
        self.assertEqual(host['Rhino'].Geometry.LineCurve.call_args.args[1].values, (0.,0.,2.))
        host['Rhino'].Geometry.Brep.CreateFromTaperedExtrude.assert_not_called()
        face.DuplicateFace.assert_not_called()
        for obj in (source,brep,output,path,mass): obj.Dispose.assert_called_once()

    def test_partial_failures_dispose_every_allocated_object(self):
        for stage in ('curve_join', 'second_wall', 'join', 'cap', 'mass'):
            with self.subTest(stage=stage):
                host, source, brep, face, output, path, mass, cap, merged, edges, profiles, walls = self.host()
                G = host['Rhino'].Geometry
                if stage == 'curve_join': G.Curve.JoinCurves.side_effect = [None]
                elif stage == 'second_wall': G.Brep.CreateFromTaperedExtrude.side_effect = [[walls[0]], None]
                elif stage == 'join': G.Brep.JoinBreps.return_value = None
                elif stage == 'cap': merged.CapPlanarHoles.return_value = None
                else: G.VolumeMassProperties.Compute.return_value = None
                with self.assertRaises(ValueError): run(request()['operations'][0], host, 1, request()['tolerance'])
                for obj in (source,brep,cap,edges[0]): obj.Dispose.assert_called_once()
                if stage != 'curve_join': profiles[0].Dispose.assert_called_once(); walls[0].Dispose.assert_called_once()
                if stage in ('cap','mass'): merged.Dispose.assert_called_once()
                if stage == 'mass': output.Dispose.assert_called_once()

    def test_uncapped_disjoint_shells_append_and_aliased_results_dispose_once(self):
        host, source, brep, face, output, path, mass, cap, merged, edges, profiles, walls = self.host()
        G = host['Rhino'].Geometry
        output.IsSolid = False
        G.Brep.__new__ = staticmethod(lambda cls: output)
        G.Brep.JoinBreps.return_value = walls
        op = copy.deepcopy(request()['operations'][0]); op['solid'] = False
        value, _ = run(op, host, 1, request()['tolerance'])
        self.assertIsNone(value['breps'][0]['volume'])
        G.Brep.JoinBreps.assert_called_once_with(walls, 1e-5)
        self.assertEqual(output.Append.call_args_list, [mock.call(w) for w in walls])
        for obj in [source,brep,cap,output]+edges+profiles+walls: obj.Dispose.assert_called_once()
        mass.Dispose.assert_not_called()

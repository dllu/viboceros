"""Surface extrusion SDK plumbing and ownership without a licensed capture."""
import copy
import json
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase, mock
from .surface_extrusion_probe import request, run, validate
ROOT = Path(__file__).resolve().parents[2]


class Vector:
    def __init__(self, values): self.values = tuple(values)
    def __neg__(self): return Vector([-x for x in self.values])
    def __imul__(self, scale): self.values = tuple(x*scale for x in self.values); return self
    def __add__(self, other): return Vector([x+y for x,y in zip(self.values,other.values)])


class SurfaceExtrusionTests(TestCase):
    def test_independent_fixtures_and_invalid_input_rejection(self):
        q = request()
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/surface_extrusion.json').read_text()))
        for operation in q['operations']: validate(operation)
        for field, value in [('face', -1), ('face', True), ('displacement', [0.,0.,0.]),
                             ('displacement', [True,0.,2.]), ('displacement', [0.,float('inf'),2.]),
                             ('solid', 1), ('both_sides', 'Yes'), ('source', dict(type='mesh')), ('extra', True)]:
            op = copy.deepcopy(q['operations'][0]); op[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError): validate(op)

    def host(self):
        class Surface: pass
        class Brep: pass
        source = Surface(); source.Dispose = mock.Mock()
        brep = Brep(); brep.Dispose = mock.Mock(); brep.Translate = mock.Mock(return_value=True)
        source.ToBrep = mock.Mock(return_value=brep)
        output, path, mass = mock.Mock(), mock.Mock(), mock.Mock()
        output.IsValid = output.IsSolid = True; output.Faces.Count = 6
        output.GetBoundingBox.return_value = NS(Min=[0.,0.,0.], Max=[2.,3.,2.])
        mass.Volume = 12.
        face = NS(CreateExtrusion=mock.Mock(return_value=output))
        class Faces(list):
            @property
            def Count(self): return len(self)
        brep.Faces = Faces([face])
        G = NS(Surface=Surface, Brep=Brep, Point3d=NS(Origin=Vector([0.,0.,0.])),
               LineCurve=mock.Mock(return_value=path), VolumeMassProperties=NS(Compute=mock.Mock(return_value=mass)))
        host = dict(Rhino=NS(Geometry=G), _object_source=lambda s,t: source,
                    _vector=Vector, _xyz=lambda p:p, _measure=lambda n,f:(f(),0))
        return host, source, brep, face, output, path, mass

    def test_both_sides_shift_private_source_and_use_a_doubled_path(self):
        host, source, brep, face, output, path, mass = self.host()
        op = copy.deepcopy(request()['operations'][0]); op['both_sides'] = True
        value, _ = run(op, host, 1, request()['tolerance'])
        self.assertEqual(brep.Translate.call_args.args[0].values, (0.,0.,-2.))
        self.assertEqual(host['Rhino'].Geometry.LineCurve.call_args.args[1].values, (0.,0.,4.))
        face.CreateExtrusion.assert_called_once_with(path, True)
        self.assertEqual(value['breps'][0]['volume'], 12.)
        for owned in (source, brep, output, path, mass): owned.Dispose.assert_called_once()

    def test_partial_failures_dispose_all_allocated_geometry(self):
        for stage in ('translate', 'extrusion', 'volume'):
            with self.subTest(stage=stage):
                host, source, brep, face, output, path, mass = self.host()
                op = copy.deepcopy(request()['operations'][0]); op['both_sides'] = True
                if stage == 'translate': brep.Translate.return_value = False
                elif stage == 'extrusion': face.CreateExtrusion.return_value = None
                else: host['Rhino'].Geometry.VolumeMassProperties.Compute.return_value = None
                with self.assertRaises(ValueError): run(op, host, 1, request()['tolerance'])
                source.Dispose.assert_called_once(); brep.Dispose.assert_called_once()
                if stage != 'translate': path.Dispose.assert_called_once()
                if stage == 'volume': output.Dispose.assert_called_once()

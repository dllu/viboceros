"""Surface/path SDK plumbing tests; mocks do not establish native Rhino parity."""
import copy
import json
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase, mock
from . import test_surface_extrusion as straight
from .surface_path_extrusion_probe import request, run, validate
ROOT = Path(__file__).resolve().parents[2]


class SurfacePathExtrusionTests(TestCase):
    def test_independent_fixtures_and_strict_options(self):
        q = request()
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/surface_path_extrusion.json').read_text()))
        for operation in q['operations']: validate(operation)
        for field, value in [('face', -1), ('face', True), ('solid', 1), ('reverse', 1),
                             ('path_length', 0), ('path_length', True), ('path_length', float('inf')),
                             ('path', {}), ('source', dict(type='mesh')), ('extra', 1)]:
            op = copy.deepcopy(q['operations'][0]); op[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError): validate(op)

    def host(self):
        host, source, brep, face, output, path, mass = straight.SurfaceExtrusionTests().host()
        path.Reverse.return_value = True
        path.GetLength.return_value = 2.
        path.LengthParameter.return_value = (True, 3.)
        path.Domain = NS(T0=2., T1=5.)
        partial = mock.Mock()
        path.Trim.return_value = partial
        host['_nurbs_curve_from_definition'] = mock.Mock(return_value=path)
        return host, source, brep, face, output, path, mass, partial

    def test_reversed_partial_path_is_owned_and_disposed(self):
        host, source, brep, face, output, path, mass, partial = self.host()
        op = copy.deepcopy(request()['operations'][0]); op.update(reverse=True, path_length=1.)
        value, _ = run(op, host, 1, request()['tolerance'])
        path.Reverse.assert_called_once(); path.LengthParameter.assert_called_once_with(1.)
        path.Trim.assert_called_once_with(2., 3.)
        face.CreateExtrusion.assert_called_once_with(partial, True)
        self.assertEqual(value['breps'][0]['volume'], 12.)
        for owned in (source, brep, output, path, mass, partial): owned.Dispose.assert_called_once()

    def test_failure_paths_dispose_owned_geometry(self):
        for stage in ('reverse', 'length', 'trim', 'extrusion', 'mass'):
            with self.subTest(stage=stage):
                host, source, brep, face, output, path, mass, partial = self.host()
                op = copy.deepcopy(request()['operations'][0]); op.update(reverse=True, path_length=1.)
                if stage == 'reverse': path.Reverse.return_value = False
                elif stage == 'length': path.LengthParameter.return_value = (False, 3.)
                elif stage == 'trim': path.Trim.return_value = None
                elif stage == 'extrusion': face.CreateExtrusion.return_value = None
                else: host['Rhino'].Geometry.VolumeMassProperties.Compute.return_value = None
                with self.assertRaises(ValueError): run(op, host, 1, request()['tolerance'])
                for owned in (source, brep, path): owned.Dispose.assert_called_once()
                if stage in ('extrusion', 'mass'): partial.Dispose.assert_called_once()
                if stage == 'mass': output.Dispose.assert_called_once()

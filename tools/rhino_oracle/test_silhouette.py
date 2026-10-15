"""Pending silhouette recipes and public SDK wiring, without fabricated captures."""
import json
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase, mock
from .silhouette_probe import request, run, validate

ROOT = Path(__file__).resolve().parents[2]


class SilhouetteTests(TestCase):
    def test_closed_recipes_match_pending_fixture_and_decline_extra_inputs(self):
        q = request()
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/silhouette_parallel.json').read_text()))
        for operation in q['operations']:
            validate(operation)
        for changes in (dict(case='script'), dict(case=[]), dict(id='x\n_Exit'),
                        dict(op='script'), dict(extra=True)):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(dict(q['operations'][0], **changes))

    def host(self):
        source, nurbs, curve = mock.Mock(), mock.Mock(), mock.Mock()
        curve.ToNurbsCurve.return_value = nurbs
        curve.GetLength.return_value = 10.
        curve.Domain.ParameterAt.side_effect = lambda t: t
        curve.PointAt.side_effect = lambda t: [2.*t, 0., 4.]
        vector = NS(Unitize=lambda: True)
        G = NS(Brep=NS(CreateFromBox=mock.Mock(return_value=source)),
               BoundingBox=mock.Mock(), Point3d=mock.Mock(), Vector3d=mock.Mock(return_value=vector),
               SilhouetteType=NS(Projecting=1, TangentProjects=2, Tangent=4, Crease=8, Boundary=16,
                                 NonSilhouetteCrease=256, NonSilhouetteTangent=512, NonSilhouetteSeam=1024),
               Silhouette=NS(Compute=mock.Mock(return_value=[NS(Curve=curve)])))
        host = dict(Rhino=NS(Geometry=G), _measure=lambda n, f: (f(), 0), _xyz=lambda p: p,
                    _nurbs_curve_definition=lambda c: dict(mock_definition=True))
        return host, source, curve, nurbs

    def test_public_sdk_camera_flags_sampling_and_owned_geometry_cleanup(self):
        host, source, curve, nurbs = self.host()
        result, _ = run(dict(op='silhouette_parallel', id='test', case='box_internal'),
                        host, 1, request()['tolerance'])
        G = host['Rhino'].Geometry
        G.Vector3d.assert_called_once_with(-1., -2., -3.)
        self.assertEqual(G.Silhouette.Compute.call_args.args[1], 31 | 256 | 512 | 1024)
        self.assertEqual(G.Silhouette.Compute.call_args.args[3:], (1e-5, 0.05))
        self.assertEqual(result['total_length'], 10.)
        self.assertEqual(len(result['curves'][0]['samples']), 33)
        source.Dispose.assert_called_once()
        curve.Dispose.assert_called_once()
        nurbs.Dispose.assert_called_once()

    def test_failure_disposes_source_and_returned_curves(self):
        host, source, curve, nurbs = self.host()
        host['_nurbs_curve_definition'] = mock.Mock(side_effect=ValueError('export failed'))
        with self.assertRaisesRegex(ValueError, 'export failed'):
            run(request()['operations'][0], host, 1, request()['tolerance'])
        source.Dispose.assert_called_once()
        curve.Dispose.assert_called_once()
        nurbs.Dispose.assert_called_once()

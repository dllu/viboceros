"""Tapered SDK recipes, option validation and ownership without a licensed capture."""
import copy
import json
import math
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase, mock
from .tapered_extrusion_probe import request, run, validate

ROOT = Path(__file__).resolve().parents[2]


class TaperedExtrusionTests(TestCase):
    def test_independent_fixture_is_stable_and_rejects_invalid_inputs(self):
        q = request()
        self.assertEqual(q, json.loads((ROOT/'tools/rhino_oracle/fixtures/tapered_extrusion.json').read_text()))
        for operation in q['operations']:
            validate(operation)
        for field, value in [('displacement', [0., 0., 0.]), ('displacement', [True, 0., 2.]),
                             ('displacement', [0., float('nan'), 2.]), ('draft_angle', 90.),
                             ('draft_angle', float('inf')), ('solid', 1), ('round_corners', 'Round'),
                             ('extra', True)]:
            operation = copy.deepcopy(q['operations'][0])
            operation[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                validate(operation)

    def host(self):
        source, original, capped, mass = (mock.Mock() for _ in range(4))
        source.IsClosed = True
        source.PointAtStart = [3., 0., 0.]
        original.CapPlanarHoles.return_value = capped
        capped.IsSolid = True
        capped.Faces.Count = 6
        capped.GetBoundingBox.return_value = NS(Min=[-3., -3., 0.], Max=[3., 3., 2.])
        mass.Volume = 42.
        direction = NS(Length=2., Unitize=mock.Mock(return_value=True))
        G = NS(ExtrudeCornerType=NS(Sharp=1, Round=2),
               Brep=NS(CreateFromTaperedExtrude=mock.Mock(return_value=[original])),
               VolumeMassProperties=NS(Compute=mock.Mock(return_value=mass)))
        host = dict(Rhino=NS(Geometry=G), _nurbs_curve_from_definition=lambda c: source,
                    _vector=lambda v: direction, _measure=lambda n, f: (f(), 0), _xyz=lambda p: p)
        return host, source, original, capped, mass, direction

    def test_sdk_uses_degrees_as_radians_caps_and_disposes_owned_geometry(self):
        host, source, original, capped, mass, direction = self.host()
        operation = request()['operations'][0]
        value, _ = run(operation, host, 1, request()['tolerance'])
        args = host['Rhino'].Geometry.Brep.CreateFromTaperedExtrude.call_args.args
        self.assertEqual(args[:4], (source, 2., direction, [3., 0., 0.]))
        self.assertAlmostEqual(args[4], math.radians(10.))
        self.assertEqual(args[5:], (1, 1e-5, 1e-8))
        self.assertEqual(value['breps'][0]['volume'], 42.)
        direction.Unitize.assert_called_once()
        for owned in (source, original, capped, mass):
            owned.Dispose.assert_called_once()

    def test_sdk_failure_disposes_sources_and_all_partial_results(self):
        for stage in ('creation', 'caps', 'integration'):
            with self.subTest(stage=stage):
                host, source, original, capped, mass, _ = self.host()
                G = host['Rhino'].Geometry
                if stage == 'creation':
                    G.Brep.CreateFromTaperedExtrude.side_effect = ValueError('creation failed')
                elif stage == 'caps':
                    original.CapPlanarHoles.return_value = None
                else:
                    G.VolumeMassProperties.Compute.return_value = None
                with self.assertRaises(ValueError):
                    run(request()['operations'][0], host, 1, request()['tolerance'])
                source.Dispose.assert_called_once()
                if stage != 'creation':
                    original.Dispose.assert_called_once()
                if stage == 'integration':
                    capped.Dispose.assert_called_once()

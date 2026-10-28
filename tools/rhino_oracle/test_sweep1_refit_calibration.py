"""Native C1 re-entry and SDK tolerance calibration without weakening acceptance."""
import gzip
import hashlib
import json
import math
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from . import sweep1_probe
from .generate_sweep1_refit_stage_reentry import request as reentry
from .generate_sweep1_refit_sweep_tolerance import request as tolerances
from .generate_sweep1_refit_closest import request as closest
from .generate_sweep1_refit_polynomial import request as polynomial

HERE = Path(__file__).resolve().parent


class SweepRefitCalibrationTests(unittest.TestCase):
    def test_native_recipes_and_producers_are_frozen(self):
        for family, request, count in [('sweep1_refit_stage_reentry', reentry(), 12),
                                       ('sweep1_refit_sweep_tolerance', tolerances(), 15),
                                       ('sweep1_refit_closest', closest(), 140),
                                       ('sweep1_refit_polynomial', polynomial(), 12)]:
            with self.subTest(family=family):
                fixture = (HERE / ('fixtures/' + family + '.json')).read_bytes()
                observed = json.loads((HERE / ('observations/' + family + '.json')).read_bytes())
                self.assertEqual(json.loads(fixture), request)
                self.assertEqual(observed['engine'], 'rhino')
                self.assertEqual(len(observed['results']), count)
                self.assertEqual([r['id'] for r in observed['results']],
                                 [op['id'] for op in request['operations']])
                self.assertEqual(hashlib.sha256(fixture).hexdigest(),
                                 observed['provenance']['request_sha256'])
                for digest in observed['provenance']['sources'].values():
                    source = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                    self.assertEqual(hashlib.sha256(source).hexdigest(), digest)

    def test_public_closest_points_lie_on_their_candidate_cubics(self):
        operations = closest()['operations']
        rows = json.loads((HERE / 'observations/sweep1_refit_closest.json').read_bytes())['results']
        self.assertEqual(len(rows), 140)
        for operation, row in zip(operations, rows):
            with self.subTest(id=operation['id']):
                value = row['value']
                t = value['parameter']
                self.assertTrue(0. <= t <= 1.)
                coefficients = [(1.-t)**3, 3.*t*(1.-t)**2, 3.*t*t*(1.-t), t**3]
                point = [sum(b*c['point'][axis] for b, c in
                             zip(coefficients, operation['control_points'])) for axis in range(3)]
                self.assertLessEqual(math.dist(point, value['point']), 1e-12)
                self.assertLessEqual(abs(math.dist(value['point'], operation['target']) -
                                         value['distance']), 1e-12)

    def test_corrected_fine_circle_parent_samples_pass_the_public_distance_test(self):
        rows = json.loads((HERE / 'observations/sweep1_refit_closest.json').read_bytes())['results']
        # These are seven public nearest-point queries per parent, not a
        # complete curve certificate or a claim about Rhino's private fitter.
        for end in ['0.0625', '0.03125', '0.015625']:
            prefix = 'nurbs-circle-1e-06-0.0-' + end + '-corrected-'
            samples = [r for r in rows if r['id'].startswith(prefix)]
            self.assertEqual(len(samples), 7)
            self.assertLessEqual(max(r['value']['distance'] for r in samples), 1e-6)

    def test_sdk_sweep_tolerance_does_not_change_the_captured_definitions(self):
        observed = json.loads((HERE / 'observations/sweep1_refit_sweep_tolerance.json').read_bytes())
        rows = observed['results']
        self.assertEqual(len(rows), 15)
        for first, second, third in zip(rows[::3], rows[1::3], rows[2::3]):
            with self.subTest(id=first['id']):
                self.assertEqual(len(first['value']), 1)
                self.assertEqual(first['value'][0]['definition'], second['value'][0]['definition'])
                self.assertEqual(first['value'][0]['definition'], third['value'][0]['definition'])

    def test_sdk_tolerance_is_independent_of_refit_and_compatible_with_ironpython(self):
        for legacy in [False, True]:
            for override in [None, 1e-9, 1e-5]:
                with self.subTest(legacy=legacy, override=override):
                    curves = [SimpleNamespace(Dispose=Mock()) for _ in range(2)]
                    brep = SimpleNamespace(Dispose=Mock())
                    create = Mock(return_value=[brep])
                    sweep = SimpleNamespace(PerformSweepRefit=create)
                    geometry = SimpleNamespace(SweepOneRail=lambda: sweep,
                        Brep=SimpleNamespace(CreateFromSweep=create),
                        Point3d=SimpleNamespace(Unset=None), Vector3d=SimpleNamespace(Unset=None),
                        SweepFrame=object(), SweepBlend=object(), SweepMiter=object(), SweepRebuild=object())
                    host = dict(Rhino=SimpleNamespace(Geometry=geometry),
                        System=SimpleNamespace(Enum=SimpleNamespace(ToObject=lambda kind, value: value)),
                        _join_close_input=Mock(side_effect=curves),
                        _sweep1_roadlike_axis=lambda *args: None,
                        _measure=lambda n, build: (build(), 0),
                        _sweep1_record=lambda *args: [{'recorded': True}])
                    operation = dict(rail={}, sections=[{}], refit_tolerance=.001, legacy_refit=legacy)
                    if override is not None:
                        operation['sdk_sweep_tolerance'] = override
                    with patch('math.isfinite', side_effect=AssertionError('unavailable in IronPython')):
                        result, _ = sweep1_probe.run(host, operation, 1, dict(absolute=1e-7, angular=1e-10))
                    self.assertEqual(result, [{'recorded': True}])
                    if legacy:
                        self.assertEqual(sweep.SweepTolerance, override or 1e-7)
                        self.assertEqual(create.call_args.args[-1], .001)
                    else:
                        self.assertEqual(create.call_args.args[9], override or 1e-7)
                        self.assertEqual(create.call_args.args[-1], .001)
                    for item in curves + [brep]:
                        item.Dispose.assert_called_once_with()

    def test_invalid_sdk_tolerances_fail_before_host_access(self):
        for value in [None, 0., -1., True, float('nan'), float('inf'), '1e-7', [], 10**1000]:
            with self.subTest(type=type(value).__name__), self.assertRaises(ValueError):
                sweep1_probe.run({}, dict(sdk_sweep_tolerance=value), 1, {})
        with self.assertRaises(ValueError):
            sweep1_probe.run({}, dict(command=True, sdk_sweep_tolerance=1e-7), 1, {})

    def test_polynomial_conversion_preserves_sdk_ownership_on_success_and_failure(self):
        for success in [True, False]:
            with self.subTest(success=success):
                class Points(list):
                    pass
                rail = SimpleNamespace(Dispose=Mock())
                profile = SimpleNamespace(Dispose=Mock(), IsRational=True,
                    Points=Points([SimpleNamespace(Weight=1.) for _ in range(4)]))
                def convert():
                    profile.IsRational = not success
                    return success
                profile.Points.MakeNonRational = Mock(side_effect=convert)
                brep = SimpleNamespace(Dispose=Mock())
                create = Mock(return_value=[brep])
                geometry = SimpleNamespace(Brep=SimpleNamespace(CreateFromSweep=create),
                    Point3d=SimpleNamespace(Unset=None), Vector3d=SimpleNamespace(Unset=None),
                    SweepFrame=object(), SweepBlend=object(), SweepMiter=object(), SweepRebuild=object())
                host = dict(Rhino=SimpleNamespace(Geometry=geometry),
                    System=SimpleNamespace(Enum=SimpleNamespace(ToObject=lambda kind, value: value)),
                    _join_close_input=Mock(side_effect=[rail, profile]),
                    _sweep1_roadlike_axis=lambda *args: None,
                    _measure=lambda n, build: (build(), 0),
                    _sweep1_record=lambda *args: [{'recorded': True}])
                operation = dict(rail={}, sections=[dict(type='nurbs',
                    control_points=[dict(weight=1.) for _ in range(4)])],
                    refit_tolerance=.001, polynomial_profiles=True)
                if success:
                    result, _ = sweep1_probe.run(host, operation, 1, dict(absolute=1e-7))
                    self.assertEqual(result, [{'recorded': True}])
                    self.assertFalse(profile.IsRational)
                    brep.Dispose.assert_called_once_with()
                else:
                    with self.assertRaises(ValueError):
                        sweep1_probe.run(host, operation, 1, dict(absolute=1e-7))
                    create.assert_not_called()
                    brep.Dispose.assert_not_called()
                profile.Points.MakeNonRational.assert_called_once_with()
                rail.Dispose.assert_called_once_with()
                profile.Dispose.assert_called_once_with()

    def test_polynomial_conversion_rejects_weighted_or_command_inputs_before_host_access(self):
        for value in [None, 0, 'true', []]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                sweep1_probe.run({}, dict(polynomial_profiles=value), 1, {})
        for sections in [None, [], [dict(type='line')],
                         [dict(type='nurbs', control_points=[])],
                         [dict(type='nurbs', control_points=[dict(weight=2.)])]]:
            with self.subTest(sections=sections), self.assertRaises(ValueError):
                sweep1_probe.run({}, dict(polynomial_profiles=True, sections=sections), 1, {})
        with self.assertRaises(ValueError):
            sweep1_probe.run({}, dict(polynomial_profiles=True, command=True), 1, {})

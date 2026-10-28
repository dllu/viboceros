"""Public Sweep API diagnostic provenance and native geometry ownership."""
import gzip
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock

from . import sweep1_probe
from .generate_sweep1_refit_legacy_diagnostics import request

HERE = Path(__file__).resolve().parent


class LegacySweepRefitTests(unittest.TestCase):
    def test_frozen_native_definitions_match_across_public_apis(self):
        family = 'sweep1_refit_legacy_diagnostics'
        fixture = (HERE / ('fixtures/' + family + '.json')).read_bytes()
        observed = json.loads((HERE / ('observations/' + family + '.json')).read_bytes())
        self.assertEqual(json.loads(fixture), request())
        self.assertEqual(observed['engine'], 'rhino')
        self.assertEqual(hashlib.sha256(fixture).hexdigest(),
                         observed['provenance']['request_sha256'])
        for digest in observed['provenance']['sources'].values():
            source = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(source).hexdigest(), digest)
        rows = observed['results']
        self.assertEqual(len(rows), 12)
        self.assertEqual([row['id'] for row in rows],
                         [op['id'] for op in request()['operations']])
        for modern, legacy in zip(rows[::2], rows[1::2]):
            with self.subTest(id=modern['id']):
                self.assertEqual(len(modern['value']), 1)
                self.assertEqual(len(legacy['value']), 1)
                self.assertEqual(modern['value'][0]['definition'],
                                 legacy['value'][0]['definition'])

    def test_legacy_sdk_selects_overloads_and_disposes_repeated_results(self):
        for count in [1, 2]:
            with self.subTest(count=count):
                curves = [SimpleNamespace(Dispose=Mock()) for _ in range(count + 1)]
                breps = [SimpleNamespace(Dispose=Mock()) for _ in range(2)]
                sweep = SimpleNamespace(PerformSweepRefit=Mock(side_effect=[[b] for b in breps]))
                host = dict(Rhino=SimpleNamespace(Geometry=SimpleNamespace(SweepOneRail=lambda: sweep)),
                            System=object(), _join_close_input=Mock(side_effect=curves),
                            _sweep1_roadlike_axis=lambda *args: None,
                            _measure=lambda n, build: ([build() for _ in range(n)], 0),
                            _sweep1_record=lambda *args: [{'recorded': True}])
                result, _ = sweep1_probe.run(
                    host, dict(rail={}, sections=[{}] * count,
                               refit_tolerance=.001, legacy_refit=True, blend=1),
                    2, dict(absolute=1e-7, angular=1e-10))
                self.assertEqual(result, [{'recorded': True}])
                expected = curves[1] if count == 1 else curves[1:]
                self.assertEqual(sweep.PerformSweepRefit.call_args.args,
                                 (curves[0], expected, .001))
                self.assertEqual(sweep.PerformSweepRefit.call_count, 2)
                self.assertEqual((sweep.SweepTolerance, sweep.AngleToleranceRadians,
                                  sweep.ClosedSweep, sweep.GlobalShapeBlending, sweep.MiterType),
                                 (1e-7, 1e-10, False, True, 0))
                for geometry in curves + breps:
                    geometry.Dispose.assert_called_once_with()

    def test_invalid_legacy_requests_fail_before_host_access(self):
        for options in [dict(legacy_refit='yes'), dict(refit_tolerance=None),
                        dict(command=True), dict(roadlike=True),
                        dict(roadlike_axis=[0, 0, 1]),
                        dict(start_point=[0, 0, 0]), dict(end_point=[0, 0, 5])]:
            with self.subTest(options=options), self.assertRaises(ValueError):
                sweep1_probe.run({}, dict(dict(legacy_refit=True, refit_tolerance=.001),
                                         **options), 1, {})

"""Frozen Refit captures, SDK ownership and validation before Rhino access."""
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock

from . import sweep1_probe
from .generate_sweep1_refit import request
from .generate_sweep1_refit_contexts import request as contexts
from .generate_sweep1_refit_fit_diagnostics import request as fit_diagnostics
from .generate_sweep1_refit_holdouts import request as holdouts

HERE = Path(__file__).resolve().parent


class SweepRefitTests(unittest.TestCase):
    def test_native_requests_and_archived_producers_are_frozen(self):
        commands = request(True)
        commands['operations'] = [op for op in commands['operations']
                                  if op['refit_tolerance'] >= .001]
        cases = [
            ('sweep1_refit_sdk', request(), 20),
            ('sweep1_refit_commands', commands, 14),
            ('sweep1_refit_sdk_holdouts', holdouts(), 8),
            ('sweep1_refit_command_holdouts', holdouts(True), 4),
            ('sweep1_refit_contexts', contexts(), 24),
            ('sweep1_refit_fit_diagnostics', fit_diagnostics(), 16),
        ]
        for family, expected, count in cases:
            with self.subTest(family=family):
                fixture = (HERE / ('fixtures/' + family + '.json')).read_bytes()
                observed = json.loads((HERE / ('observations/' + family + '.json')).read_bytes())
                self.assertEqual(json.loads(fixture), expected)
                self.assertEqual(observed['engine'], 'rhino')
                self.assertEqual(len(observed['results']), count)
                self.assertEqual([op['id'] for op in expected['operations']],
                                 [row['id'] for row in observed['results']])
                self.assertEqual(hashlib.sha256(fixture).hexdigest(),
                                 observed['provenance']['request_sha256'])
                for digest in observed['provenance']['sources'].values():
                    source = gzip.decompress((HERE / ('observations/source_snapshots/' + digest + '.gz')).read_bytes())
                    self.assertEqual(hashlib.sha256(source).hexdigest(), digest)

    def test_refit_sdk_arguments_and_geometry_ownership(self):
        for fit in [None, 0., 1e-9, .001, .1]:
            with self.subTest(fit=fit):
                curves = [SimpleNamespace(Dispose=Mock()) for _ in range(2)]
                brep = SimpleNamespace(Dispose=Mock())
                create = Mock(return_value=[brep])
                geometry = SimpleNamespace(
                    Brep=SimpleNamespace(CreateFromSweep=create),
                    Point3d=SimpleNamespace(Unset=None), Vector3d=SimpleNamespace(Unset=None),
                    SweepFrame=object(), SweepBlend=object(), SweepMiter=object(), SweepRebuild=object())
                host = dict(Rhino=SimpleNamespace(Geometry=geometry),
                            System=SimpleNamespace(Enum=SimpleNamespace(ToObject=lambda kind, value: value)))
                host.update(_join_close_input=Mock(side_effect=curves),
                            _sweep1_roadlike_axis=lambda *args: None,
                            _measure=lambda n, build: (build(), 0),
                            _sweep1_record=lambda *args: [{'recorded': True}])
                result, _ = sweep1_probe.run(host, dict(rail={}, sections=[{}], refit_tolerance=fit),
                                            1, dict(absolute=1e-7))
                self.assertEqual(result, [{'recorded': True}])
                self.assertEqual(create.call_args.args[-3:], (2 if fit is not None else 0, 0, fit or 0.))
                for item in [*curves, brep]:
                    item.Dispose.assert_called_once_with()

    def test_invalid_refits_fail_before_accessing_the_host(self):
        for fit in [-1., float('nan'), float('inf'), -float('inf'), True, '.01', [], {}, 10**1000]:
            with self.subTest(type=type(fit).__name__), self.assertRaises(ValueError):
                sweep1_probe.run({}, dict(refit_tolerance=fit), 1, {})
        with self.assertRaises(ValueError):
            sweep1_probe.run({}, dict(refit_tolerance=.01, rebuild_point_count=5), 1, {})

    def test_command_macro_sets_the_refit_and_preserves_other_choices(self):
        text = sweep1_probe.macro(dict(refit_tolerance=.01, refit_rail=True, blend=1), ['rail', 'profile'])
        self.assertIn('_Simplify=_Refit _RefitTolerance=', text)
        self.assertNotIn('_RebuildCount=', text)
        self.assertIn('_ShapeBlending=_Global _RefitRail=_Yes', text)

    def test_real_client_stages_the_refit_companion_for_an_isolated_job(self):
        from .structural_sdk import stage_helpers, validate_operations
        payload = request()
        validate_operations(payload)
        with tempfile.TemporaryDirectory() as directory:
            job = Path(directory)
            stage_helpers(payload, HERE, job)
            self.assertEqual({p.name for p in job.iterdir()}, {'sweep1_probe.py'})
            subprocess.run([
                sys.executable, '-I', '-c',
                'import sys;sys.path.insert(0,sys.argv[1]);import sweep1_probe;'
                'sweep1_probe.validate({"refit_tolerance":0.01});'
                'assert "_Simplify=_Refit" in sweep1_probe.macro('
                '{"refit_tolerance":0.01},["rail","section"])', str(job)],
                check=True, capture_output=True, text=True)

    def test_client_preflight_rejects_conflicting_or_invalid_refit_options(self):
        from .structural_sdk import validate_operations
        for options in [dict(refit_tolerance=-1.),
                        dict(refit_tolerance=.01, rebuild_point_count=5)]:
            payload = request()
            payload['operations'] = [dict(payload['operations'][0], **options)]
            with self.assertRaises(ValueError):
                validate_operations(payload)

    def test_captures_distinguish_single_batch_and_curve_fit_policies(self):
        def rows(family):
            return {r['id']: r['value'] for r in json.loads(
                (HERE / ('observations/' + family + '.json')).read_bytes())['results']}
        single = rows('sweep1_refit_sdk')['open-refit-0.001'][0]['definition']
        batch = rows('sweep1_refit_contexts')['equal-pair-scale-1.0-0.001'][0]['definition']
        fitted = rows('sweep1_refit_fit_diagnostics')['open-fit-0.001']
        self.assertEqual(single['control_count'][1], 4)
        self.assertEqual(batch['control_count'][1], 12)
        self.assertEqual(len(fitted['control_points']), 11)


if __name__ == '__main__':
    unittest.main()

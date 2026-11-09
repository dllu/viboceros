"""Fitting schema, helper staging, immutable inputs and native lifetimes."""
import copy
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

from . import surface_fit_probe as probe
from .generate_surface_fit import request
from .structural_sdk import stage_helpers, validate_operations


class SurfaceFitTests(unittest.TestCase):
    def test_schema_and_staging(self):
        query = request()
        original = copy.deepcopy(query)
        validate_operations(query)
        op = query['operations'][0]
        for changes in [dict(extra=True), dict(degree_u=True), dict(degree_v=0),
                        dict(degree_u=12), dict(fit_tolerance=0.), dict(fit_tolerance=-1.),
                        dict(fit_tolerance=float('nan')), dict(fit_tolerance=float('inf'))]:
            with self.assertRaises(ValueError):
                probe.validate(dict(op, **changes))
        with tempfile.TemporaryDirectory() as directory:
            stage_helpers(query, Path(__file__).parent, Path(directory))
            self.assertEqual([p.name for p in Path(directory).iterdir()], ['surface_fit_probe.py'])
        self.assertEqual(query, original)

    def test_native_failure_and_null_preserve_ownership(self):
        for stage in ['success', 'alias', 'null', 'native', 'conversion', 'invalid', 'mutation']:
            with self.subTest(stage=stage):
                candidate = SimpleNamespace(IsValid=True, Dispose=mock.Mock())
                fitted = SimpleNamespace(ToNurbsSurface=mock.Mock(return_value=candidate), Dispose=mock.Mock())
                source = SimpleNamespace(Fit=mock.Mock(return_value=fitted), Dispose=mock.Mock())
                if stage == 'alias':
                    fitted.ToNurbsSurface.return_value = fitted
                    fitted.IsValid = True
                if stage == 'null': source.Fit.return_value = None
                if stage == 'native': source.Fit.side_effect = ValueError('fit failed')
                if stage == 'conversion': fitted.ToNurbsSurface.side_effect = ValueError('conversion failed')
                if stage == 'invalid': candidate.IsValid = False
                calls = [0]
                def definition(s):
                    if s is source: calls[0] += 1
                    return dict(identity='changed' if stage == 'mutation' and calls[0] > 1 else
                                'source' if s is source else 'candidate')
                host = dict(_nurbs_surface_from_definition=lambda _: source,
                            _nurbs_surface_definition=definition,
                            _measure=lambda n, f: (f(), 17))
                if stage in ['success', 'alias', 'null']:
                    value, elapsed = probe.run(request()['operations'][0], host, 1, request()['tolerance'])
                    self.assertEqual(value['before'], value['after'])
                    self.assertEqual(value['surface'] is None, stage == 'null')
                    self.assertEqual(elapsed, 17)
                else:
                    with self.assertRaises(ValueError):
                        probe.run(request()['operations'][0], host, 1, request()['tolerance'])
                source.Dispose.assert_called_once()
                self.assertEqual(fitted.Dispose.call_count, int(stage not in ['native', 'null']))
                self.assertEqual(candidate.Dispose.call_count, int(stage in ['success', 'invalid', 'mutation']))


if __name__ == '__main__':
    unittest.main()

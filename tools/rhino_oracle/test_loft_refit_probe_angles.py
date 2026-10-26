"""Independent fits must inspect the same angular policy as the Loft call."""
import ast
import math
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

HERE = Path(__file__).resolve().parent
FLAGS = {
    'inspect_refit_clock': 'loft_refit_clock_probe',
    'inspect_refit_sets': 'loft_refit_set_probe',
    'inspect_refit_frames': 'loft_refit_frame_probe',
    'inspect_refit_sources': 'loft_refit_source_probe',
    'inspect_refit_beziers': 'loft_refit_probe',
}


class LoftRefitProbeAngleTests(unittest.TestCase):
    def run_loft(self, explicit, inspect=True):
        curve = Mock()
        curve.Fit.return_value = Mock()
        factory = Mock(return_value=[])
        geometry = SimpleNamespace(
            LoftType=SimpleNamespace(**{s.capitalize(): s for s in
                ['normal', 'loose', 'tight', 'straight', 'uniform']}),
            Point3d=SimpleNamespace(Unset=None),
            Brep=SimpleNamespace(CreateFromLoft=factory, CreateFromLoftRefit=factory))
        doc = SimpleNamespace(ModelAngleToleranceRadians=1e-8,
                              ModelAbsoluteTolerance=1e-9)
        scope = dict(math=math, Rhino=SimpleNamespace(Geometry=geometry,
            RhinoDoc=SimpleNamespace(ActiveDoc=doc)),
            _nurbs_curve_from_definition=lambda _: curve,
            _nurbs_curve_definition=lambda _: {'source': True},
            _measure=lambda count, build: (build(), 0.))
        tree = ast.parse((HERE / 'rhino_worker.py').read_text())
        function = next(f for f in tree.body if isinstance(f, ast.FunctionDef)
                        and f.name == '_loft')
        exec(compile(ast.Module(body=[function], type_ignores=[]), 'loft-probes', 'exec'), scope)
        operation = dict(curves=[{}], refit_tolerance=.001,
                         inspect_inputs=True, allow_empty=True)
        if explicit is not None:
            operation['refit_angle_tolerance'] = explicit
        modules = {}
        for flag, name in FLAGS.items():
            if inspect:
                operation[flag] = True
            modules[name] = SimpleNamespace(inspect=Mock(return_value={}),
                                            cubic_sections=Mock(return_value=[]))
        operation['inspect_refitted'] = inspect
        with patch.dict('sys.modules', modules):
            value, _ = scope['_loft'](operation, 1)
        curve.Dispose.assert_called_once()
        self.assertEqual(value['before'], value['after'])
        return value, curve, factory, modules

    def test_default_and_explicit_angles_reach_every_inspection(self):
        for explicit in [None, 0., 1e-8, .2, math.pi]:
            with self.subTest(angle=explicit):
                expected = math.pi / 180. if explicit is None else explicit
                value, curve, factory, modules = self.run_loft(explicit)
                self.assertEqual(value['refit_probe_angle_tolerance'], expected)
                curve.Fit.assert_called_once_with(3, .001, expected)
                for name, module in modules.items():
                    function = module.cubic_sections if name == 'loft_refit_probe' else module.inspect
                    self.assertEqual(function.call_count, 1)
                    self.assertEqual(function.call_args.args[2], expected)
                arguments = factory.call_args.args
                self.assertEqual(len(arguments), 6 if explicit is None else 7)
                self.assertEqual(arguments[-1], .001)
                if explicit is not None:
                    self.assertEqual(arguments[-2], expected)

    def test_plain_capture_has_no_probe_fields_or_calls(self):
        value, curve, _, modules = self.run_loft(None, inspect=False)
        self.assertNotIn('refit_probe_angle_tolerance', value)
        curve.Fit.assert_not_called()
        for module in modules.values():
            module.inspect.assert_not_called()
            module.cubic_sections.assert_not_called()


if __name__ == '__main__':
    unittest.main()

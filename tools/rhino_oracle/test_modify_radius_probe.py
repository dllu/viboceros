"""Bounded recipes are validated before launching the native command."""
import unittest
from .modify_radius_probe import validate, FAMILIES, MODES
from .structural_sdk import PROBES, validate_operations


class ModifyRadiusRecipes(unittest.TestCase):
    def recipe(self, **kw):
        result = dict(op='modify_radius_command', id='arc', family='arc', mode='Radius', value=4., artifact_path='/tmp/source.3dm')
        result.update(kw)
        return result

    def test_all_families_and_measurements(self):
        for family in FAMILIES:
            for mode in MODES:
                validate(self.recipe(family=family, mode=mode, domain=[-2., 9.], copy=True, preselect=False, copies=3, mixed=True))

    def test_rejects_unbounded_and_ambiguous_inputs(self):
        for kw in [dict(value=v) for v in (True, 0., -1., float('nan'), float('inf'), 1e7)]+[
            dict(family='line'), dict(mode='Length'), dict(id='../bad'), dict(copy=1), dict(preselect=1),
            dict(copies=4), dict(copies=True), dict(domain=[1., 1.]), dict(domain=[0., float('inf')]),
            dict(domain=[False, 1.]), dict(delta=float('nan')), dict(extra=True)]:
            with self.subTest(kw=kw), self.assertRaises(ValueError):
                validate(self.recipe(**kw))

    def test_registered_host_validator_and_companions(self):
        self.assertIn('modify_radius_command', PROBES)
        validate_operations(dict(operations=[self.recipe()]))
        with self.assertRaises(ValueError):
            validate_operations(dict(operations=[self.recipe(value=-3.)]))


if __name__ == '__main__':
    unittest.main()

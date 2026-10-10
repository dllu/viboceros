import unittest
from .smlib_reference_probe import CASES, validate


class SmlibReferenceProbeTests(unittest.TestCase):
    def test_fixed_recipes_validate(self):
        for case in CASES:
            validate(dict(op='smlib_reference', id=case, case=case))

    def test_invalid_and_unbounded_recipes_are_rejected(self):
        for changes in [dict(case=[]), dict(case='unknown'), dict(id=''), dict(id='x' * 81), dict(extra=True)]:
            operation = dict(op='smlib_reference', id='box', case='box')
            operation.update(changes)
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(operation)

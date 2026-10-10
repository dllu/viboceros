import unittest
from .fillet_edge_probe import CASES, validate


class FilletEdgeProbeTests(unittest.TestCase):
    def test_fixed_sdk_recipe_validation(self):
        for case in CASES:
            validate(dict(op='fillet_edge_reference', id=case, case=case))
        for changes in [dict(case=[]), dict(case='unknown'), dict(id=''), dict(radius=1.)]:
            op = dict(op='fillet_edge_reference', id='owned', case='box-single')
            op.update(changes)
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(op)

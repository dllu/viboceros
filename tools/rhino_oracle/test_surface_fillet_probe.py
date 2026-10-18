import copy
import json
import math
from pathlib import Path
import unittest
from .surface_fillet_probe import validate


class SurfaceFilletRequestTests(unittest.TestCase):
    def setUp(self):
        fixtures = Path(__file__).parent / 'fixtures'
        self.operations = []
        for name in ['surface_fillet.json', 'surface_fillet_picks.json', 'surface_fillet_parents.json']:
            self.operations.extend(json.loads((fixtures / name).read_text())['operations'])

    def test_saved_sdk_recipes_and_explicit_side_choices_validate(self):
        for op in self.operations:
            validate(op)
            explicit = dict(op, sides=[1, -1])
            validate(explicit)
            validate(dict(op, trim=True))
            validate(dict(op, trim=False))

    def test_malformed_geometry_and_side_picks_are_rejected_before_launch(self):
        op = self.operations[0]
        cases = [None, {}, dict(op, radius=0), dict(op, radius=math.inf), dict(op, sides=[True, 1]),
                 dict(op, sides=[0, 1]), dict(op, picks=None), dict(op, picks=[[0, math.nan], [0, 1]]),
                 dict(op, queries=[]), dict(op, surfaces=None), dict(op, trim=None),
                 dict(op, trim=1), dict(op, trim='Split')]
        broken = copy.deepcopy(op)
        broken['surfaces'][0]['knots_u'] = None
        cases.append(broken)
        broken = copy.deepcopy(op)
        broken['surfaces'][0]['control_points'][0]['weight'] = 0
        cases.append(broken)
        for case in cases:
            with self.subTest(case=case):
                with self.assertRaises(ValueError):
                    validate(case)

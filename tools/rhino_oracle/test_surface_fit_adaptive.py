"""Reproducible discovery inputs and immutable knot-refinement recipes."""
import copy
import json
import unittest
from pathlib import Path
from . import generate_surface_fit_adaptive as adaptive
from . import generate_surface_fit_strips as strips
from .structural_sdk import validate_operations


class SurfaceFitAdaptiveTests(unittest.TestCase):
    def test_generators_reproduce_every_captured_input(self):
        for family, module, count in [('surface_fit_adaptive', adaptive, 40), ('surface_fit_strips', strips, 36)]:
            query=module.request();validate_operations(query)
            captured=json.loads((Path(__file__).parent/'fixtures'/(family+'.json')).read_text())
            self.assertEqual(query,captured)
            self.assertEqual(len(query['operations']),count)
            self.assertEqual(len(set(o['id'] for o in query['operations'])),count)

    def test_refinement_and_direction_changes_do_not_mutate_sources(self):
        source=adaptive.bezier(True);before=copy.deepcopy(source)
        refined=adaptive.dense(source)
        self.assertEqual(source,before)
        self.assertEqual([refined['control_point_count_u'],refined['control_point_count_v']],[11,11])
        source=copy.deepcopy(refined)
        normal=strips.strip(refined);swapped=strips.strip(refined,True)
        self.assertEqual(refined,source)
        self.assertEqual([normal['control_point_count_u'],normal['control_point_count_v']],[11,2])
        self.assertEqual([swapped['control_point_count_u'],swapped['control_point_count_v']],[2,11])
        self.assertEqual(normal['knots_u'],swapped['knots_v'])


if __name__=='__main__':
    unittest.main()

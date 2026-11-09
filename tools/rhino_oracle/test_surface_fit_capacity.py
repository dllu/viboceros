"""Owned paired inputs and native observations; no fitting-policy shortcuts."""
import copy
import gzip
import json
from pathlib import Path
import unittest
from . import generate_surface_fit_capacity as pilot
from . import generate_surface_fit_capacity_holdouts as holdouts
from .structural_sdk import validate_operations

HERE=Path(__file__).parent
def read(family):
    p=HERE/'observations'/(family+'.json.gz')
    return json.loads(gzip.decompress(p.read_bytes())if p.exists()else p.with_suffix('').read_bytes())


class SurfaceFitCapacityTests(unittest.TestCase):
    def test_generators_reproduce_both_frozen_requests(self):
        for family,module,count in [('surface_fit_capacity',pilot,38),('surface_fit_capacity_holdouts',holdouts,19)]:
            q=module.request();before=copy.deepcopy(q);validate_operations(q)
            self.assertEqual(len(q['operations']),count)
            self.assertEqual(q,json.loads((HERE/'fixtures'/(family+'.json')).read_bytes()))
            self.assertEqual(q,before)

    def test_distinct_knots_change_results_at_equal_stored_control_counts(self):
        q=pilot.request();by_id={o['id']:o for o in q['operations']}
        rows={r['id']:r['value'] for r in read('surface_fit_capacity')['results']}
        a='fit-capacity-rational-bezier-uv-mid-triple-0.01'
        b='fit-capacity-rational-bezier-uv-three-spans-0.01'
        for axis in ['u','v']:
            self.assertEqual(by_id[a]['surface']['control_point_count_'+axis],7)
            self.assertEqual(by_id[b]['surface']['control_point_count_'+axis],7)
        self.assertIsNone(rows[a]['surface']);self.assertEqual(rows[b]['surface']['control_count'],[5,5])
        a='fit-capacity-polynomial-uv-new-triple-0.01';b='fit-capacity-polynomial-uv-three-new-0.01'
        self.assertEqual(by_id[a]['surface']['control_point_count_u'],12)
        self.assertEqual(by_id[b]['surface']['control_point_count_u'],12)
        self.assertIsNone(rows[a]['surface']);self.assertEqual(rows[b]['surface']['control_count'],[7,7])

    def test_all_holdout_degrees_distinguish_multiplicity_from_distinct_spans(self):
        rows={r['id']:r['value'] for r in read('surface_fit_capacity_holdouts')['results']}
        for label,count in [('2-1',[5,2]),('2-2',[4,4]),('3-2',[5,4]),('4-3',[6,5])]:
            prefix='fit-capacity-holdout-'+label+'-'
            for plan in ['base','one','multiple']:self.assertIsNone(rows[prefix+plan]['surface'])
            self.assertEqual(rows[prefix+'distinct']['surface']['control_count'],count)


if __name__=='__main__':unittest.main()

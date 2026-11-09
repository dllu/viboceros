"""Keep captured-basis proofs and unknown policy relationships explicit."""
import copy
import json
import unittest
from . import report_surface_fit_capacity as record


class SurfaceFitCapacityRecordTests(unittest.TestCase):
    def test_frozen_record_reproduces(self):
        self.assertEqual(record.report(),json.loads((record.ROOT/'docs/surface-fit-capacity-validation.json').read_bytes()))

    def test_budget_shape_requires_an_axis_reduction_after_the_initial_patch(self):
        self.assertTrue(record.admits([4,4],[4,4],[4,4]))
        self.assertTrue(record.admits([5,5],[4,4],[7,5]))
        self.assertFalse(record.admits([5,5],[4,4],[5,5]))
        self.assertFalse(record.admits([5,4],[4,4],[4,7]))

    def test_false_accuracy_and_degree_change_eligibility_are_rejected(self):
        captures=record.data(record.NEW);prior=record.data(record.PRIOR)
        log=record.read(record.PROOF/'portable.log.gz').decode();rows=record.rows(log,'FIT_CAPACITY')
        def replace(changed):
            values=iter(changed)
            return '\n'.join('FIT_CAPACITY '+json.dumps(next(values))if s.startswith('FIT_CAPACITY ')else s for s in log.splitlines())
        altered=copy.deepcopy(rows);next(r for r in altered if r['native_bound']is not None)['native_bound']=1e-8
        with self.assertRaises(AssertionError):record.summarize(captures,prior,replace(altered))
        altered=copy.deepcopy(rows);next(r for r in altered if 'lower-degree'in r['id'])['candidate_admitted']=True
        with self.assertRaises(AssertionError):record.summarize(captures,prior,replace(altered))


if __name__=='__main__':unittest.main()

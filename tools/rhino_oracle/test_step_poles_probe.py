import unittest
from .step_poles_probe import CASES, validate


class StepPolesProbeTests(unittest.TestCase):
    def test_fixed_bounded_artifact_paths(self):
        for case in CASES:
            validate(dict(op='step_poles', id=case, case=case,
                          artifact_path='/tmp/viboceros-step-poles-owned/' + case + '.step'))

    def test_invalid_paths_cases_and_fields(self):
        for changes in [dict(case=[]), dict(case='unknown'), dict(id=''), dict(extra=True),
                        dict(artifact_path='/etc/sphere.step'),
                        dict(artifact_path='/tmp/viboceros-step-poles-owned/sphere.step\n'),
                        dict(artifact_path='/tmp/viboceros-step-poles-owned/sub/sphere.step'),
                        dict(artifact_path='/tmp/viboceros-step-poles-owned/../sphere.step')]:
            operation = dict(op='step_poles', id='sphere', case='sphere',
                             artifact_path='/tmp/viboceros-step-poles-owned/sphere.step')
            operation.update(changes)
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate(operation)

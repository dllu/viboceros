import unittest
from .delete_subcurve_probe import validate, FAMILIES
from .structural_sdk import validate_operations

class DeleteSubcurveRecipes(unittest.TestCase):
    def recipe(self, **kw):
        op=dict(op='delete_subcurve_command',id='test',family='line',start=.2,end=.7,artifact_path='/tmp/source.3dm')
        op.update(kw);return op
    def test_known_families_and_numeric_directions(self):
        for family in FAMILIES:
            for length in(-3.,0.,3.):validate(self.recipe(family=family,length=length,preselect=False,domain=[-3.,7.],reverse=True))
    def test_invalid_recipes_fail_before_native_execution(self):
        for kw in(dict(start=True),dict(start=-1.),dict(end=float('nan')),dict(end=2.),dict(length=float('inf')),dict(length=101.),dict(length=True),dict(domain=[2.,1.]),dict(domain=[0.,float('inf')]),dict(reverse=1),dict(family='bad'),dict(id='../bad'),dict(extra=True),dict(length_fraction=True),dict(length_fraction=-1.),dict(length_fraction=1.,length=3.),dict(confirmation=1)):
            with self.subTest(kw=kw),self.assertRaises(ValueError):validate(self.recipe(**kw))
    def test_registered_host_validation(self):
        validate_operations(dict(operations=[self.recipe()]))
        with self.assertRaises(ValueError):validate_operations(dict(operations=[self.recipe(length=-101.)]))
if __name__=='__main__':unittest.main()

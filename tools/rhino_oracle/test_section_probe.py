"""Section input validation precedes native geometry/document access."""
import unittest
from .section_probe import validate
class SectionProbeTests(unittest.TestCase):
    def fixture(self):return dict(op='section_command',id='case',sources=[dict(type='line',start=[0,-1,0],end=[0,1,0])],start=[-1,0,0],end=[1,0,0])
    def test_finite_points_and_boolean_options_reject_invalid_values(self):
        for field,value in [('start',[0,0]),('end',[float('inf'),0,0]),('extend',1),('group','yes'),('properties','invalid')]:
            f=self.fixture();f[field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):validate(f)
    def test_source_count_and_unknown_fields_are_bounded(self):
        for values in [[],[{}]*65]:
            f=self.fixture();f['sources']=values
            with self.assertRaises(ValueError):validate(f)
        f=self.fixture();f['unknown']=1
        with self.assertRaises(ValueError):validate(f)

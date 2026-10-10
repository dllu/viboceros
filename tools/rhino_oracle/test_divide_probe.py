import unittest
from .divide_probe import validate
class DivideProbeTests(unittest.TestCase):
    def fixture(self):return dict(op='divide_command',id='line',sources=[dict(type='line',start=[0,0,0],end=[10,0,0])],mode='count',value=4)
    def test_bounds_types_and_options_are_checked_before_launch(self):
        for key,value in [('sources',[]),('sources',[{}]*65),('mode','bad'),('value',0),('value',float('inf')),('value',True),('value',2.5),('split','yes'),('unknown',1)]:
            f=self.fixture();f[key]=value
            with self.subTest(key=key),self.assertRaises(ValueError):validate(f)
    def test_valid_curve_modes_and_boolean_options(self):
        for mode in ('count','length','chord'):
            f=self.fixture();f.update(mode=mode,split=True,group=True,mark_ends=False,delete_remainder=True);validate(f)

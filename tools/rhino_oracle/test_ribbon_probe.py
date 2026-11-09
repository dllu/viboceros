"""Validate owned capture inputs before starting a private Rhino process."""
import copy
import unittest
from .ribbon_probe import validate
from .generate_ribbon import request as ribbons
from .generate_curve_loose_offset import request as offsets
from .generate_curve_loose_offset_types import request as types
from .generate_ribbon_polyline import request as polylines
from .generate_ribbon_corner_correspondence import request as correspondence
from .generate_ribbon_smooth_angles import request as angles
from .generate_ribbon_sharp_holdouts import request as sharp
from .generate_ribbon_sharp_seams import request as seams


class RibbonProbeTests(unittest.TestCase):
    def test_all_authored_operations_validate(self):
        for factory in [ribbons,offsets,types,polylines,correspondence,angles,sharp,seams]:
            for op in factory()['operations']:validate(op)

    def test_nonfinite_boolean_and_zero_distances_are_rejected(self):
        for value in [True,0.,-1.,float('nan'),float('inf')]:
            op=copy.deepcopy(offsets()['operations'][0]);op['distance']=value
            with self.assertRaises(ValueError):validate(op)

    def test_unknown_choices_and_arbitrary_macro_tokens_are_rejected(self):
        for key,value in [('corner','Sharp _Delete'),('output_layer','Input _Cancel'),
                          ('native_kind','unknown'),('side',[0.,float('inf'),0.]),('loose','Yes')]:
            op=copy.deepcopy(offsets()['operations'][0]);op[key]=value
            with self.assertRaises(ValueError):validate(op)


if __name__=='__main__':unittest.main()

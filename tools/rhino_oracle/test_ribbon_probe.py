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
from .generate_ribbon_smooth import request as smooth
from .generate_ribbon_analytic import request as analytic
from .generate_ribbon_near_analytic import request as near
from .generate_ribbon_workflows import request as workflows
from .generate_ribbon_cplane import request as cplane
from .generate_ribbon_cplane_boundaries import request as boundaries
from .generate_ribbon_cplane_policy import request as policy
from .generate_ribbon_cplane_switches import request as switches
from .generate_ribbon_cplane_thresholds import request as thresholds
from .generate_ribbon_cplane_holdouts import request as holdouts


class RibbonProbeTests(unittest.TestCase):
    def test_all_authored_operations_validate(self):
        for factory in [ribbons,offsets,types,polylines,correspondence,angles,sharp,seams,smooth,analytic,near,workflows,
                        cplane,boundaries,policy,switches,thresholds,holdouts]:
            for op in factory()['operations']:validate(op)

    def test_nonfinite_boolean_and_zero_distances_are_rejected(self):
        for value in [True,0.,-1.,float('nan'),float('inf')]:
            op=copy.deepcopy(offsets()['operations'][0]);op['distance']=value
            with self.assertRaises(ValueError):validate(op)

    def test_unknown_choices_and_arbitrary_macro_tokens_are_rejected(self):
        for key,value in [('corner','Sharp _Delete'),('output_layer','Input _Cancel'),
                          ('native_kind','unknown'),('side',[0.,float('inf'),0.]),('loose','Yes'),('postselected','Yes')]:
            op=copy.deepcopy(offsets()['operations'][0]);op[key]=value
            with self.assertRaises(ValueError):validate(op)

    def test_fit_tolerance_and_construction_plane_are_validated(self):
        for key, values in [
            ('fit_tolerance', [True, -1., float('nan'), float('inf'), '_Delete']),
            ('model_angle_tolerance', [True, 0., -1., 4., float('nan')]),
            ('cplane', [[], [[0.,0.,0.],[1.,0.,0.],[2.,0.,0.]],
                        [[0.,0.,0.],[1.,0.,0.],[0.,float('inf'),0.]]])]:
            for value in values:
                op=copy.deepcopy(cplane()['operations'][0]);op[key]=value
                with self.assertRaises(ValueError):validate(op)
        op=copy.deepcopy(cplane()['operations'][0]);op['loose']=True
        with self.assertRaises(ValueError):validate(op)


if __name__=='__main__':unittest.main()

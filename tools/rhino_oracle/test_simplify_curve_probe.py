import tempfile
from pathlib import Path
from unittest import TestCase
from .simplify_curve_probe import validate
from .structural_sdk import validate_operations,stage_helpers
class SimplifyCurveProbeTests(TestCase):
    def source(self):return dict(op='simplify_curve_command',id='sample',family='nurbs_line',artifact_path='/tmp/owned-simplify.3dm')
    def test_bounded_family_domain_selection_and_complete_staging(self):
        op=self.source();op.update(copies=3,preselect=False,domain=[-2.,8.],mixed=True)
        q=dict(protocol_version=1,iterations=1,operations=[op]);validate_operations(q)
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(q,Path(__file__).parent,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()},{'simplify_curve_probe.py','join_probe.py','merge_edges_probe.py'})
    def test_invalid_recipes_reject_before_host_access(self):
        for key,value in [('copies',True),('copies',0),('copies',4),('preselect',1),('mixed',0),('domain',[1.,0.]),('domain',[0.,float('inf')]),('delta',float('nan')),('delta',-.1),('family','unknown'),('id','two words'),('artifact_path','not-3dm')]:
            op=self.source();op[key]=value
            with self.subTest(key=key),self.assertRaises(ValueError):validate(op)
    def test_unrecognized_fields_reject(self):
        op=self.source();op['unknown']=True
        with self.assertRaises(ValueError):validate(op)

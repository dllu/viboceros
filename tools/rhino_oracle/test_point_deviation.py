import copy,json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase
from .point_deviation_probe import validate
from .structural_sdk import stage_helpers

class PointDeviationTests(TestCase):
    def request(self):return json.loads(Path(__file__).with_name('fixtures').joinpath('point_deviation.json').read_text())
    def test_recipes_use_supported_original_geometry_and_public_sdk_only(self):
        q=self.request();self.assertEqual(len(q['operations']),14)
        for op in q['operations']:validate(op)
        self.assertEqual({o['targets'][0]['type']for o in q['operations']},{'line','arc','circle','ellipse','polyline','polycurve','nurbs','surface','box_brep','mesh_brep','mesh'})
    def test_invalid_coordinates_and_point_targets_reject(self):
        op=self.request()['operations'][0]
        for key,value in [('points',[[True,0,0]]),('points',[[float('nan'),0,0]]),('points',[]),('targets',[dict(type='point',point=[0,0,0])]),('targets',[])]:
            with self.subTest(key=key),self.assertRaises(ValueError):validate(dict(op,**{key:value}))
    def test_standalone_job_imports_without_pathlib(self):
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(self.request(),Path(__file__).parent,Path(directory))
            script="import sys;sys.path.insert(0,sys.argv[1]);sys.modules['pathlib']=None;import point_deviation_probe;point_deviation_probe.validate(__import__('json').loads(sys.argv[2]))"
            subprocess.run([sys.executable,'-I','-c',script,directory,json.dumps(self.request()['operations'][0])],check=True)

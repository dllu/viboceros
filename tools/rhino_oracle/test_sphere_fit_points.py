import copy,json,os,subprocess,sys,tempfile
from fractions import Fraction as F
from pathlib import Path
from unittest import TestCase,mock
from .sphere_fit_points_probe import request,validate
from .structural_sdk import stage_helpers
from .client import OracleClient,OracleProtocolError

class SphereFitPointsTests(TestCase):
    def test_native_commands_retain_filters_degeneracy_and_cap_source_witness(self):
        q=request();r=json.loads(Path(__file__).with_name('observations').joinpath('sphere_fit_points_rhino_20261016.json').read_text())
        self.assertEqual(len(r['results']),21);self.assertEqual(sum(len(x['value']['outputs'])for x in r['results']),16)
        rows={x['id']:x['value']for x in r['results']}
        for op,row in zip(q['operations'],r['results']):
            validate(op);self.assertEqual(op['id'],row['id'])
            for out in row['value']['outputs']:
                self.assertEqual(out['definition']['degree'],[2,2]);self.assertEqual(out['definition']['control_count'],[9,5]);self.assertEqual(out['layer'],'SphereFitCurrent');self.assertFalse(out['selected']);self.assertIsNone(out['name'])
        for label in ['axes-cloud','axes-mesh','two-points']:self.assertFalse(rows[label]['command_returned']);self.assertFalse(any(s['selected']for s in rows[label]['sources']))
        cap=next(o['points']for o in q['operations']if o['id']=='shallow-cap');s=rows['shallow-cap']['sdk'];epsilon=F(1e-6)
        self.assertTrue(all((F(2)-F(1e-12))**2<=sum(F(x)**2 for x in p)<=(F(2)+F(1e-12))**2 for p in cap))
        self.assertTrue(any(sum((F(x)-F(c))**2 for x,c in zip(p,s['center']))>(F(s['radius'])+epsilon)**2 for p in cap))

    def test_bad_requests_reject_before_launch(self):
        op=request()['operations'][0]
        for key,value in [('source','surface'),('points',[]),('points',[[True,0,0]]),('extra',True)]:
            candidate=copy.deepcopy(op);candidate[key]=value
            with self.subTest(key=key),self.assertRaises(ValueError):validate(candidate)
        with(mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),mock.patch('tools.rhino_oracle.client._run_logged')as launch,self.assertRaises(OracleProtocolError)):
            OracleClient(launcher='/bin/true').run_rhino(request())
        launch.assert_not_called()

    def test_shared_fitting_helpers_import_from_the_standalone_job(self):
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(request(),Path(__file__).parent,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()},{'sphere_fit_points_probe.py','line_through_points_probe.py','plane_through_points_probe.py'})
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import sphere_fit_points_probe',directory],check=True)

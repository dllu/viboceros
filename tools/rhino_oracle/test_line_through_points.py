"""Private native line-fit recipes and shared worker staging."""
import copy,json,os,subprocess,sys,tempfile
from pathlib import Path
from unittest import TestCase,mock
from .line_through_points_probe import request,validate
from .client import OracleClient,OracleProtocolError
from .structural_sdk import stage_helpers


class LineThroughPointsTests(TestCase):
    def test_retained_commands_and_sdk_cover_short_lines_and_coincident_noops(self):
        req=request();response=json.loads(Path(__file__).with_name('observations').joinpath('point_fitting_rhino_20261016.json').read_text())
        rows={r['id']:r['value']for r in response['results']}
        self.assertEqual(len(req['operations']),25);self.assertEqual(len(rows),42)
        for op in req['operations']:
            validate(op);row=rows[op['id']]
            self.assertTrue(all(s['present']for s in row['sources']))
            self.assertEqual(row['sdk']['created'],bool(row['outputs']))
            for obj in row['outputs']:
                self.assertEqual(obj['definition']['degree'],1)
                self.assertEqual(len(obj['definition']['control_points']),2)
                self.assertEqual(obj['layer'],'LineFitCurrent');self.assertIsNone(obj['name']);self.assertFalse(obj['selected'])
                self.assertEqual(obj['groups'],[]);self.assertEqual(obj['color_source'],'ColorFromLayer')
                self.assertEqual(obj['endpoints'],row['sdk']['endpoints'])
        self.assertTrue(rows['line-coincident']['command_returned']);self.assertFalse(rows['line-coincident']['outputs'])
        self.assertTrue(rows['line-coincident']['sources'][0]['selected']);self.assertFalse(rows['line-coincident-post']['sources'][0]['selected'])
        self.assertEqual(rows['line-below-model-tolerance']['outputs'][0]['definition']['domain'],[0.,1e-9])

    def test_invalid_requests_reject_before_launch_and_two_curve_grips_are_supported(self):
        base=request()['operations'][0]
        for field,value in [('source','surface'),('source',False),('points',[]),('points',[[True,0,0]]),('postselect',1),('extra',True)]:
            op=copy.deepcopy(base);op[field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):validate(op)
        validate(dict(base,source='curve_grips',points=[[0,0,0],[1,0,0]]))
        with (mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),mock.patch('tools.rhino_oracle.client._run_logged')as launch,self.assertRaises(OracleProtocolError)):
            OracleClient(launcher='/bin/true').run_rhino(request())
        launch.assert_not_called()

    def test_shared_probe_dependencies_import_in_an_isolated_interpreter(self):
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(request(),Path(__file__).parent,Path(directory))
            self.assertEqual({p.name for p in Path(directory).iterdir()},{'line_through_points_probe.py','plane_through_points_probe.py'})
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import line_through_points_probe',directory],check=True)

"""Validate owned fit recipes, retained public output and private host staging."""
import copy
import json
import math
import os
from pathlib import Path
from unittest import TestCase,mock
from .client import OracleClient,OracleProtocolError
from .plane_through_points_probe import request,validate
from .structural_sdk import stage_helpers


class PlaneThroughPointsTests(TestCase):
    def test_retained_commands_cover_sources_and_degenerate_sets(self):
        req=request()
        response=json.loads(Path(__file__).with_name('observations').joinpath('plane_through_points_rhino_20261016.json').read_text())
        self.assertEqual(response['engine'],'rhino')
        self.assertEqual(len(response['results']),17)
        for op,row in zip(req['operations'],response['results']):
            validate(op)
            self.assertEqual(op['id'],row['id'])
            self.assertTrue(all(s['present'] for s in row['value']['sources']))
            expected=op['id'] not in ('one-cloud','two-cloud','coincident-cloud')
            self.assertEqual(len(row['value']['outputs']),int(expected))
            self.assertEqual(row['value']['command_returned'],expected)
            for out in row['value']['outputs']:
                self.assertEqual(out['definition']['degree'],[1,1])
                self.assertEqual(out['definition']['control_count'],[2,2])
                self.assertEqual(out['layer'],'PlaneFitCurrent')
                self.assertEqual(out['color_source'],'ColorFromLayer')
                self.assertFalse(out['selected']);self.assertIsNone(out['name']);self.assertEqual(out['groups'],[])
                corners=out['corners']
                self.assertTrue(all(math.isfinite(v)for p in corners for v in p))
                for edge,domain in [(math.dist(corners[0],corners[1]),out['definition']['domain_u']),
                                    (math.dist(corners[0],corners[2]),out['definition']['domain_v'])]:
                    self.assertAlmostEqual(edge,domain[1]-domain[0],delta=1e-8)

    def test_invalid_recipes_reject_before_launch(self):
        original=request()['operations'][0]
        for field,value in [('points',[[True,0,0]]),('points',[[float('nan'),0,0]]),('points',[]),('source','mesh'),('postselect',1),('extra',True)]:
            op=copy.deepcopy(original);op[field]=value
            with self.subTest(field=field,value=value),self.assertRaises(ValueError):validate(op)
        with (mock.patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),
              mock.patch('tools.rhino_oracle.client._run_logged')as launch,
              self.assertRaises(OracleProtocolError)):
            OracleClient(launcher='/bin/true').run_rhino(request())
        launch.assert_not_called()

    def test_actual_staged_probe_imports_in_an_isolated_interpreter(self):
        import subprocess,sys,tempfile
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(request(),Path(__file__).parent,Path(directory))
            self.assertEqual([p.name for p in Path(directory).iterdir()],['plane_through_points_probe.py'])
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import plane_through_points_probe',directory],check=True)

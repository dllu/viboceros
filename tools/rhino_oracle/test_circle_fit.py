"""Closed owned Circle fitting inputs and unmodified native lifecycle evidence."""
import copy
import hashlib
import json
import math
import os
from pathlib import Path
from unittest import TestCase, mock
from .circle_fit_probe import request, validate
from .client import OracleClient, OracleProtocolError

ROOT=Path(__file__).resolve().parents[2]

class CircleFitTests(TestCase):
    def test_closed_request_factory_and_rejections(self):
        fixture=json.loads((ROOT/'tools/rhino_oracle/fixtures/circle_fit_points.json').read_text())
        self.assertEqual(fixture,request())
        for op in fixture['operations']:validate(op)
        base=copy.deepcopy(fixture['operations'][0])
        for updates in (dict(extra=True),dict(id='x\n_Delete'),dict(points=[]),dict(points=[[0,0,0]]*513),dict(points=[[False,0,0]]*3),dict(points=[[float('nan'),0,0]]*3),dict(points=[[float('inf'),0,0]]*3),dict(points=[[1000001,0,0]]*3)):
            with self.subTest(updates=updates),self.assertRaises(ValueError):validate(dict(base,**updates))

    def test_private_display_scheme_and_one_iteration_before_launch(self):
        for scheme,display,headless,count in ((None,':201',':201',1),('VibocerosFit',':0',None,1),('VibocerosFit',':201',':201',2),('VibocerosFit',':201',':201',True)):
            q=request();q['iterations']=count;env={'DISPLAY':display}
            if headless:env['VIBOCEROS_ORACLE_HEADLESS']=headless
            with mock.patch.dict(os.environ,env,clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((ValueError,OracleProtocolError)):OracleClient(settings_scheme=scheme).run_rhino(q,1)
                launch.assert_not_called()

    def test_native_circle_and_sdk_geometry_selection_and_history(self):
        r=json.loads((ROOT/'tools/rhino_oracle/observations/circle_fit_points.json').read_text())
        self.assertEqual(r['engine_version'],'8.32.26160.13001')
        self.assertEqual([o['id'] for o in request()['operations']],[row['id'] for row in r['results']])
        for row in r['results']:
            v=row['value'];self.assertTrue(v['success'])
            self.assertEqual(len([e for e in v['events'] if e['name']=='Circle']),1)
            self.assertTrue(all(o['selected'] for o in v['after'] if o['kind']=='point'))
            self.assertTrue(all(not o['selected'] for o in v['redo']))
            circles=[o for o in v['after'] if o['kind']=='circle']
            if v['sdk']['radius']==0:
                self.assertEqual(circles,[]);self.assertEqual(v['undo'],[])
            else:
                self.assertEqual(len(circles),1);self.assertEqual(circles[0]['circle'],v['sdk'])
                self.assertEqual(v['undo'],v['before'])
            for key in ('origin','x','y','normal'):
                self.assertTrue(all(math.isfinite(c) for c in v['sdk'][key]))
        self.assertEqual(r['results'][21]['value']['sdk']['radius'],199999999.99999982)

    def test_benchmark_uses_bounded_prebuilt_array_inputs(self):
        from .circle_fit_benchmark import request as benchmark_request, validate as benchmark_validate
        q=benchmark_request()
        self.assertEqual(q,json.loads((ROOT/'tools/rhino_oracle/fixtures/circle_fit_benchmark.json').read_text()))
        benchmark_validate(q['operations'][0])
        for changes in (dict(counts=[1,1000000]),dict(id='x'),dict(extra=True)):
            with self.assertRaises(ValueError):benchmark_validate(dict(q['operations'][0],**changes))
        r=json.loads((ROOT/'tools/rhino_oracle/observations/circle_fit_benchmark.json').read_text())
        self.assertEqual(r['engine_version'],'8.32.26160.13001')
        rows=r['results'][0]['value']['records']
        self.assertEqual([row['count'] for row in rows],[100,10000])
        for row in rows:
            self.assertEqual(len(row['milliseconds']),3)
            self.assertTrue(all(math.isfinite(t) and t>0 for t in row['milliseconds']))
            self.assertEqual(row['median_ms'],sorted(row['milliseconds'])[1])
        with mock.patch.dict(os.environ,{'DISPLAY':':201','VIBOCEROS_ORACLE_HEADLESS':':201'},clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(q,1)
            launch.assert_not_called()

    def test_provenance_hashes(self):
        r=json.loads((ROOT/'docs/circle-fit-provenance.json').read_text())
        self.assertTrue(r['private_xvfb']);self.assertEqual(r['native_recipes'],22)
        self.assertFalse(r['full_native_parity'])
        for path,expected in r['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),expected,path)

"""Closed Taper recipes, private displays and retained public native evidence."""
import hashlib
import json
import os
import math
import random
import struct
from pathlib import Path
import unittest
from unittest.mock import patch
from . import taper_command_probe as geometry, taper_options_probe as preferences
from .client import OracleClient, OracleError, OracleProtocolError
from .number_token import number_token

ROOT=Path(__file__).parent
RECIPES=[('taper_geometry_command',geometry.request),('taper_fitting_command',geometry.fitting_request),
         ('taper_command_followup',geometry.followup_request),('taper_command_boundary',geometry.boundary_request),
         ('taper_command_threshold',geometry.threshold_request),('taper_identity_command',geometry.identity_request),
         ('taper_options_command',preferences.request)]

class TaperCommandTests(unittest.TestCase):
    def test_macro_tokens_roundtrip_cutoffs_extremes_and_ieee_samples(self):
        samples=[0.,-0.,2.**-32,math.nextafter(2.**-32,math.inf),math.nextafter(2.**-32,0.),1e-320,1e308,-1e308]
        rng=random.Random(20261003)
        samples.extend(struct.unpack('!d',rng.randbytes(8))[0] for _ in range(1000))
        for value in samples:
            if not math.isfinite(value):
                with self.assertRaises(ValueError):number_token(value)
                continue
            token=number_token(value)
            self.assertLessEqual(len(token),25)
            self.assertEqual(struct.pack('!d',float(token)),struct.pack('!d',value))
        for value in [float('nan'),float('inf'),float('-inf')]:
            with self.assertRaises(ValueError):number_token(value)

    def test_retained_requests_are_closed_bounded_factories(self):
        for name,factory in RECIPES:
            request=factory()
            self.assertEqual(request,json.loads((ROOT/'fixtures'/f'{name}.json').read_text()))
            probe=preferences if name=='taper_options_command' else geometry
            for op in request['operations']:probe.validate(op)

    def test_distances_and_commands_reject_nonfinite_and_injected_values(self):
        original=geometry.request()['operations'][0]
        for key,values in [('id',[True,'unsafe\n_Delete']),('shape',[None,'Curve _Delete']),
            ('axis',[None,[],'Z _Delete']),('cplane',['WorldXY _Delete']),
            ('start_distance',[True,'2 _Delete',[True,0,0],[0,0],[float('inf'),0,0],101,float('nan'),10**500]),
            ('end_distance',[None,float('inf'),'1 _Delete']),('tolerance',[True,0,1e-13,float('nan'),.1]),
            ('targets',[[],[True],[101]]),('finish',['Complete _Delete']),('extra',[True])
        ]+[(k,[1,'Yes']) for _,k in geometry.FLAGS]+[('grouped',[1])]:
            for value in values:
                with self.subTest(key=key,value=value),self.assertRaises(ValueError):geometry.validate(dict(original,**{key:value}))
        geometry.validate(dict(original,copy=True,targets=[-1,[0,2,10]]))

    def test_preferences_reject_unbounded_or_unknown_actions(self):
        original=preferences.request()['operations'][0]
        for steps in [[],[dict(kind='Delete')],[dict(kind='New',script='_Delete')],
                      [dict(kind='RememberCopyOptions',enabled=1)],
                      [dict(original['steps'][0],options={'Rigid':1})],
                      [dict(original['steps'][0],options={'Unknown':True})],
                      [dict(original['steps'][0],shape='Box',options={'PreserveStructure':True})],
                      [dict(original['steps'][0],pending_options={'Flat':True})]]:
            with self.subTest(steps=steps),self.assertRaises(ValueError):preferences.validate(dict(original,steps=steps))

    def test_private_scheme_and_one_iteration_precede_launch(self):
        for factory in (geometry.request,preferences.request):
            for scheme,iterations in [(None,1),('VibocerosOracleTest',True),('VibocerosOracleTest',2)]:
                req=factory();req['iterations']=iterations
                with patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}),patch('tools.rhino_oracle.client._run_logged') as launch,self.assertRaises(OracleProtocolError):
                    OracleClient(launcher='/bin/true',settings_scheme=scheme).run_rhino(req)
                launch.assert_not_called()

    def test_shared_desktop_cannot_launch_taper(self):
        for factory in (geometry.request,preferences.request):
            with patch.dict(os.environ,{'DISPLAY':':0','VIBOCEROS_ORACLE_HEADLESS':':101'}),patch('tools.rhino_oracle.client._run_logged') as launch,self.assertRaisesRegex(OracleError,'dedicated Xvfb'):
                OracleClient(launcher='/bin/true',settings_scheme='VibocerosOracleTest').run_rhino(factory())
            launch.assert_not_called()

    def test_raw_geometry_and_terminal_history_are_complete(self):
        count=0
        for name,factory in RECIPES[:-1]:
            res=json.loads((ROOT/'observations'/f'{name}.json').read_text())
            self.assertEqual(res['engine_version'],'8.32.26160.13001')
            self.assertEqual(len(res['results']),len(factory()['operations']))
            for op,row in zip(factory()['operations'],res['results']):
                count+=1;self.assertEqual(op['id'],row['id']);v=row['value']
                events=[e for e in v['events'] if e['name']=='Taper'];self.assertEqual(len(events),1)
                self.assertIn(events[0]['result'],('Success','Cancel'))
                self.assertIn('Start distance',v['history']);self.assertEqual(events[0]['objects'],v['after'])
                if v['undo'] is not None:
                    self.assertEqual([o['geometry'] for o in v['undo']['objects']],[o['geometry'] for o in v['before']['objects']])
                    self.assertEqual(v['redo']['groups'],v['after']['groups'])
                    self.assertEqual([o['geometry'] for o in v['redo']['objects']],[o['geometry'] for o in v['after']['objects']])
                else:
                    self.assertEqual([o['geometry'] for o in v['after']['objects']],[o['geometry'] for o in v['before']['objects']])
        self.assertEqual(count,90)

    def test_every_flag_waits_for_success_and_preferences_survive_new(self):
        res=json.loads((ROOT/'observations/taper_options_command.json').read_text())
        records=res['results'][0]['value']['records'];self.assertEqual(len(records),33)
        for record in records:
            step=record['step'];before=record['query_before']['defaults'];after=record['query_after']['defaults']
            if step['kind']=='Taper':
                success=next(e['result'] for e in record['result']['events'] if e['name']=='Taper')=='Success'
                if not success:self.assertEqual(before,after)
                else:
                    edits=dict(step['options'],**step.get('pending_options',{}))
                    for name,value in edits.items():
                        if name!='Copy':self.assertEqual(after[name],value)
            elif step['kind'] in ('Undo','Redo','New'):self.assertEqual(before,after)
            for key in ('query_before','query_after'):
                self.assertEqual(set(record[key]['defaults']),set(preferences.OPTIONS))
                self.assertIn('Start distance',record[key]['history'])

    def test_provenance_and_precision_diagnostics_remain_unmodified(self):
        repo=ROOT.parents[1]
        provenance=json.loads((repo/'docs/taper-command-provenance.json').read_text())
        for path,digest in provenance['sha256'].items():
            self.assertEqual(hashlib.sha256((repo/path).read_bytes()).hexdigest(),digest,path)
        initial=json.loads((ROOT/'observations/taper_command_boundary_precision_initial.json').read_text())
        exact=json.loads((ROOT/'observations/taper_command_boundary.json').read_text())
        result=lambda r,i:next(e['result'] for e in r['results'][i]['value']['events'] if e['name']=='Taper')
        self.assertEqual(result(initial,3),'Success');self.assertEqual(result(exact,3),'Cancel')

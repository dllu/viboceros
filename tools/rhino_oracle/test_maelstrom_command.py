"""Closed native Maelstrom commands, private displays and public witnesses."""
import hashlib
import json
import os
from pathlib import Path
import unittest
from unittest.mock import patch
from . import maelstrom_command_probe as geometry, maelstrom_options_probe as preferences
from .client import OracleClient, OracleError, OracleProtocolError

ROOT = Path(__file__).parent
RECIPES = [('maelstrom_geometry_command',geometry.request),
           ('maelstrom_fitting_command',geometry.fitting_request),
           ('maelstrom_identity_command',geometry.identity_request),
           ('maelstrom_fit_correspondence',geometry.correspondence_request),
           ('maelstrom_radius_command',geometry.radius_request),
           ('maelstrom_options_command',preferences.request)]


class MaelstromCommandTests(unittest.TestCase):
    def test_closed_factories_match_retained_requests(self):
        for name,factory in RECIPES:
            request=factory()
            self.assertEqual(request,json.loads((ROOT/'fixtures'/(name+'.json')).read_text()))
            for op in request['operations']:
                (preferences if 'options' in name else geometry).validate(op)

    def test_geometry_rejects_unbounded_nonfinite_and_injected_values(self):
        base=geometry.request()['operations'][0]
        for key,values in [('id',[True,'bad\n_Delete']),('shape',[None,'Points _Delete']),
                           ('radius0',[True,'2 _Delete',[True,0,0],[0,0],[0,0,0],float('nan'),10**500]),
                           ('radius1',[None,float('inf'),[0,0,101],[0,0,5],[0,0,0]]),('normal',[[0,0,0],[0,0,float('inf')]]),
                           ('origin',[[0,0,101]]),('degrees',[True,float('nan'),1441]),
                           ('tolerance',[True,0,1e-13,.1]),('copy',[1]),('rigid',['Yes']),
                           ('grouped',[1]),('angles',[[],[True],[float('inf')]]),
                           ('undo_unchanged',[1]),('diagnose_fit',[1,True]),('extra',[True])]:
            for value in values:
                with self.subTest(key=key,value=value),self.assertRaises(ValueError):
                    geometry.validate(dict(base,**{key:value}))
        geometry.validate(dict(base,radius1=-5.))
        geometry.validate(dict(base,radius1=0.))
        with self.assertRaises(ValueError):geometry.validate(dict(base,radius0=0.))

    def test_preference_workflows_are_closed_and_bounded(self):
        base=preferences.request()['operations'][0]
        for steps in [[],[dict(kind='Delete')],[dict(kind='New',script='_Delete')],
                      [dict(kind='RememberCopyOptions',enabled=1)],
                      [dict(base['steps'][0],options={'Rigid':1})],
                      [dict(base['steps'][0],options={'PreserveStructure':True})],
                      [dict(base['steps'][0],radius0=True)],
                      [dict(base['steps'][0],radius0=2.123456)],
                      [dict(base['steps'][0],radius0=float('inf'))]]:
            with self.subTest(steps=steps),self.assertRaises(ValueError):
                preferences.validate(dict(base,steps=steps))

    def test_private_display_scheme_and_single_iteration_precede_launch(self):
        for _,factory in RECIPES:
            for scheme,iterations in [(None,1),('VibocerosOracleTest',True),('VibocerosOracleTest',2)]:
                req=factory();req['iterations']=iterations
                with patch.dict(os.environ,{'DISPLAY':':101','VIBOCEROS_ORACLE_HEADLESS':':101'}), \
                     patch('tools.rhino_oracle.client._run_logged') as launch,self.assertRaises(OracleProtocolError):
                    OracleClient(launcher='/bin/true',settings_scheme=scheme).run_rhino(req)
                launch.assert_not_called()
            with patch.dict(os.environ,{'DISPLAY':':0','VIBOCEROS_ORACLE_HEADLESS':':101'}), \
                 patch('tools.rhino_oracle.client._run_logged') as launch,self.assertRaisesRegex(OracleError,'dedicated Xvfb'):
                OracleClient(launcher='/bin/true',settings_scheme='VibocerosOracleTest').run_rhino(factory())
            launch.assert_not_called()

    def test_terminal_geometry_and_history_have_all_witnesses(self):
        count=0
        for name,factory in RECIPES[:-1]:
            r=json.loads((ROOT/'observations'/(name+'.json')).read_text())
            self.assertEqual(r['engine_version'],'8.32.26160.13001')
            self.assertEqual(len(r['results']),len(factory()['operations']))
            for op,row in zip(factory()['operations'],r['results']):
                count+=1;self.assertEqual(op['id'],row['id']);v=row['value']
                event=[e for e in v['events'] if e['name']=='Maelstrom']
                self.assertEqual(len(event),1)
                self.assertIn(event[0]['result'],('Success','Cancel'))
                self.assertEqual(event[0]['objects'],v['after'])
                self.assertIn('Radius',v['history'])
                if event[0]['result']=='Success':
                    multiplier=2+len(op.get('angles',[])) if op['copy'] else 1
                    self.assertEqual(len(v['after']['objects']),len(v['before']['objects'])*multiplier)
                if v['undo'] is not None:
                    self.assertEqual(v['undo']['objects'],v['before']['objects'])
                    original_groups=len(v['before']['groups'])
                    self.assertEqual(v['undo']['groups'][:original_groups],v['before']['groups'])
                    self.assertTrue(all(g['members']==[] for g in v['undo']['groups'][original_groups:]))
                    expected=v['after']
                    if any(e['name']=='Cancel' for e in v['events']) and event[0]['result']=='Success':
                        # Radius diagnostics end a potentially rejected getter.
                        # If it succeeded, that later Cancel owns selection cleanup.
                        expected=json.loads(json.dumps(expected))
                        for obj in expected['objects']:obj['selected']=False
                    self.assertEqual(v['redo'],expected)
                if op.get('undo_unchanged'):
                    self.assertIsNotNone(v['undo'])
        self.assertEqual(count,85)

    def test_native_fitting_floor_and_getter_preference_transitions(self):
        rows=json.loads((ROOT/'observations/maelstrom_fitting_command.json').read_text())['results']
        for i in range(0,12,3):
            self.assertEqual(rows[i+1]['value']['after'],rows[i+2]['value']['after'])
        records=json.loads((ROOT/'observations/maelstrom_options_command.json').read_text())['results'][0]['value']['records']
        self.assertEqual(len(records),29)
        copy,remember,first_radius=False,True,1.
        for row in records:
            expected=dict(Copy=copy if remember else False,Rigid=False)
            self.assertEqual(row['query_before']['defaults'],expected)
            self.assertEqual(row['query_before']['first_radius'],first_radius)
            step=row['step']
            if step['kind']=='RememberCopyOptions':remember=step['enabled']
            if step['kind']=='Maelstrom':
                first_radius=step.get('radius0',2.)
                if 'Copy' in step['options']:copy=step['options']['Copy']
                if 'Copy' in step.get('pending_options',{}):copy=step['pending_options']['Copy']
            if not remember:copy=False
            self.assertEqual(row['query_after']['defaults'],dict(Copy=copy,Rigid=False))
            self.assertEqual(row['query_after']['first_radius'],first_radius)

    def test_provenance_hashes_cover_helpers_and_raw_data(self):
        repo=ROOT.parents[1]
        p=json.loads((repo/'docs/maelstrom-command-provenance.json').read_text())
        for path,digest in p['sha256'].items():
            self.assertEqual(hashlib.sha256((repo/path).read_bytes()).hexdigest(),digest,path)


if __name__=='__main__':unittest.main()

import copy
import json
import math
from pathlib import Path
from unittest import TestCase

from .flow_line_comparison import comparison
from .flow_line_probe import validate
from .flow_roadlike_probe import validate as validate_roadlike

BASE=Path(__file__).parent


class FlowLocalTests(TestCase):
    def read(self,path):
        return json.loads((BASE/path).read_text())

    def data(self,name):
        return [self.read(path) for path in ['fixtures/'+name+'.json',
            'observations/'+name+'_viboceros_20261017.json',
            'observations/'+name+'_rhino_20261017.json']]

    def test_all_native_local_workflows_match_geometry_owners_selection_groups_and_history(self):
        for name,count in [('flow_local',50),('flow_local_owners',76),('flow_local_edges',42)]:
            q,a,n=self.data(name)
            for op,row in zip(q['operations'],n['results']):
                (validate_roadlike if op['op']=='flow_roadlike'else validate)(op)
                self.assertEqual(op['id'],row['id'])
                v=row['value']
                for key in ['command_returned','undo_command_returned','redo_command_returned']:
                    self.assertIs(v[key],True)
                self.assertNotIn('_Cancel',v['macro'])
                self.assertNotIn('Unknown command',v['history'])
                self.assertNotIn('Invalid',v['history'])
            r=comparison(q,a,n)
            self.assertEqual(r,self.read('observations/'+name+'_comparison_20261017.json'))
            self.assertEqual(r['cases'],count)
            self.assertEqual(r['matched_cases'],count)

    def test_native_translation_uses_cubic_distance_falloff_with_finite_end_caps(self):
        cases=[]
        q,_,n=self.data('flow_local')
        cases.append((q['operations'][0],n['results'][0]))
        q,_,n=self.data('flow_local_edges')
        cases.extend(zip(q['operations'][:2],n['results'][:2]))
        for op,row in cases:
            inner,outer=sorted(op['local_radii'])
            source=op['source']['points'];points=row['value']['after_script']['outputs'][0]['points']
            self.assertEqual(len(points),len(source))
            for p,actual in zip(source,points):
                x,y,z=p
                distance=math.hypot(max(0,-x,x-10),y,z)
                t=max(0,min(1,(outer-distance)/(outer-inner)))
                expected=[x,y+10*t*t*(3-2*t),z]
                self.assertLess(max(abs(a-b)for a,b in zip(actual,expected)),1e-12)
        self.assertEqual(cases[0][1]['value']['after_script']['outputs'],cases[1][1]['value']['after_script']['outputs'])

    def test_unchanged_clouds_are_copied_but_outside_grip_owners_are_not(self):
        q,_,n=self.data('flow_local_edges')
        values={r['id']:r['value']for r in n['results']}
        for family in ['cloud','control','edit','mesh']:
            for rigid in [0,1]:
                v=values['local-outside-'+family+'-1'+str(rigid)]
                expected=v['before']['object_count']+(family=='cloud')
                self.assertEqual(v['after_script']['object_count'],expected)
                if family!='cloud':
                    self.assertEqual(v['before'],v['after_script'])
                else:
                    self.assertEqual([o['points']for o in v['after_script']['outputs']],[v['before']['outputs'][0]['points']]*2)
        q,a,n=self.data('flow_local_edges')
        i=next(i for i,o in enumerate(q['operations'])if o['id']=='local-outside-control-10')
        damaged=copy.deepcopy(n);state=damaged['results'][i]['value']['after_script']
        duplicate=copy.deepcopy(state['outputs'][0]);duplicate['original']=False
        state['outputs'].append(duplicate);state['object_count']+=1
        self.assertEqual(comparison(q,a,damaged)['matched_cases'],41)

    def test_finite_and_nonfinite_geometry_corruption_cannot_pass(self):
        q,a,n=self.data('flow_local')
        for value in [.01,float('nan')]:
            damaged=copy.deepcopy(a)
            damaged['results'][0]['value']['after']['outputs'][0]['points'][0][0]=value
            self.assertEqual(comparison(q,damaged,n)['matched_cases'],49)

    def test_radius_request_validation_bounds_values_but_accepts_either_order_and_zero(self):
        op=self.read('fixtures/flow_local.json')['operations'][0]
        for value in [None,[],[1],[1,2,3],[1,1],[-1,2],[True,2],[float('nan'),3],[0,float('inf')],[0,1e7]]:
            changed=copy.deepcopy(op);changed['local_radii']=value
            with self.assertRaises(ValueError):validate(changed)
        for value in [[0,3],[3,0],[3,1],[1,3]]:
            changed=copy.deepcopy(op);changed['local_radii']=value;validate(changed)

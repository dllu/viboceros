"""Actual command evidence, distinct from the legacy grip-Delete SDK matrix."""
import copy
import gzip
import hashlib
import json
import tempfile
from collections import Counter
from pathlib import Path
from unittest import TestCase
from .generate_remove_control_point import request
from .remove_control_point_probe import validate
from .structural_sdk import stage_helpers

HERE = Path(__file__).parent


class RemoveControlPointCommands(TestCase):
    def check_capture(self, name, q):
        capture = json.loads((HERE / ('observations/' + name + '.json')).read_text())
        self.assertEqual([r['id'] for r in capture['results']], [o['id'] for o in q['operations']])
        for row, op in zip(capture['results'], q['operations']):
            a,b = dict(row['value']['recipe']),dict(op)
            self.assertEqual(Path(a.pop('artifact_path')).name,Path(b.pop('artifact_path')).name)
            self.assertEqual(a,b)
            p=Path(op['artifact_path'])
            self.assertEqual(hashlib.sha256(p.read_bytes()).hexdigest(),capture['provenance']['fixtures'][p.name])
        for name,digest in capture['provenance']['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=digest and name in ('rhino_worker.py','structural_sdk.py'):
                data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)
        return capture

    def test_neighborhoods_reproduce_exact_input_discontinuities(self):
        from .generate_remove_control_point_neighbors import request as neighbors
        q=neighbors();self.assertEqual(len(q['operations']),120)
        c=self.check_capture('remove_control_point_neighbors',q)
        rows={r['id']:r['value']for r in c['results']}
        changed=lambda v:v['before'][0]['geometry']!=v['after'][0]['geometry']
        for group in range(6):
            prefix='center_'+str(group)+'_'
            a,b=rows[prefix+'exact_a'],rows[prefix+'exact_b']
            self.assertEqual(a['after'][0]['geometry'],b['after'][0]['geometry'])
            self.assertEqual(changed(a),group>=3)
            for scale in range(3):
                for axis in range(3):
                    for sign in ('minus','plus'):
                        v=rows[prefix+'s%d_a%d_%s'%(scale,axis,sign)]
                        self.assertTrue(v['success'])
                        expected=(sign=='plus' if axis==0 else sign=='minus')if group==3 else True
                        self.assertEqual(changed(v),expected,v['recipe']['id'])

    def test_public_parameter_queries_and_helpers(self):
        from .generate_remove_control_point_parameters import request as parameters
        q=parameters();self.assertEqual(len(q['operations']),10)
        c=self.check_capture('remove_control_point_parameters',q)
        for r in c['results']:
            q=r['value']['parameter_queries']
            self.assertTrue(q['surface_closest']['found'])
            self.assertTrue(q['face_closest']['found'])
            self.assertTrue(q['unit_surface']['found'])
            self.assertEqual(len(q['curves']),10)
            self.assertEqual(len(q['unit_curves']),2)
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(parameters(),HERE,Path(d))
            self.assertEqual(len(list(Path(d).iterdir())),6)

    def test_composites_parent_charts_and_parameter_traces(self):
        from .generate_remove_control_point_composites import request as composites
        from .generate_remove_control_point_composite_parameters import request as traces
        q=composites();self.assertEqual(len(q['operations']),22)
        c=self.check_capture('remove_control_point_composites',q)
        expected={r['id']:r['value']for r in c['results']}
        tq=traces();self.assertEqual(len(tq['operations']),19)
        t=self.check_capture('remove_control_point_composite_parameters',tq)
        for r in t['results']:
            v=r['value'];old=expected[r['id']]
            for key in ('before','after'):
                self.assertEqual(v[key][0]['geometry'],old[key][0]['geometry'],(r['id'],key))
            self.assertEqual(v['success'],old['success'])
            self.assertEqual(len(v['parameter_queries']['segments']),2)
        for r in c['results']:
            v=r['value'];old=v['before'][0];new=v['after'][0]
            self.assertEqual(old['geometry_text'],new['geometry_text'],r['id'])
            for key in ('name','layer','groups','user_text'):
                self.assertEqual(old[key],new[key],(r['id'],key))
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(tq,HERE,Path(d))
            self.assertEqual(len(list(Path(d).iterdir())),6)

    def test_bounded_actions_and_exact_staged_helpers(self):
        q = request()
        self.assertEqual(len(q['operations']), 133)
        self.assertEqual(len({o['id'] for o in q['operations']}), 133)
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(q, HERE, Path(d))
            self.assertEqual({p.name for p in Path(d).iterdir()}, {
                'remove_control_point_probe.py', 'remove_knot_command_probe.py',
                'insert_kink_probe.py', 'join_probe.py', 'merge_edges_probe.py'})
            for p in Path(d).iterdir():
                self.assertEqual(p.read_bytes(), (HERE / p.name).read_bytes())
        base = q['operations'][0]
        for actions in (None, [True], ['Undo'], ['!'], ['Delete'], [dict(grip=-1)],
                        [dict(grip=True)], [dict(grip=[1, 2, 3])], [dict(point=[0., 0., float('nan')])],
                        [dict(point=[True, 0., 0.])], [dict(grip=1)] * 2):
            with self.subTest(actions=actions), self.assertRaises(ValueError):
                validate(dict(copy.deepcopy(base), actions=actions))

    def test_frozen_native_results_and_source_hashes(self):
        capture = json.loads((HERE / 'observations/remove_control_point_command.json').read_text())
        q = request()
        self.assertEqual([r['id'] for r in capture['results']], [o['id'] for o in q['operations']])
        outcomes = Counter()
        for row, op in zip(capture['results'], q['operations']):
            v = row['value']
            a, b = dict(v['recipe']), dict(op)
            self.assertEqual(Path(a.pop('artifact_path')).name, Path(b.pop('artifact_path')).name)
            self.assertEqual(a, b)
            events = [e for e in v['events'] if e['name'] == 'RemoveControlPoint']
            self.assertEqual(len(events), 1, row['id'])
            outcomes[events[0]['result']] += 1
            self.assertEqual(v['success'], events[0]['result'] == 'Success')
            self.assertTrue(v['macro'].startswith('_RemoveControlPoint '))
            self.assertLessEqual(len(v['picks']), 1)
            if op['family'] not in ('line', 'box'):
                self.assertNotIn('Unknown command:', v['history'], row['id'])
            self.assertFalse(v['after'][0]['selected'])
            for key in ('name', 'layer', 'groups', 'user_text'):
                self.assertEqual(v['after'][0][key], v['before'][0][key], (row['id'], key))
            p = Path(op['artifact_path'])
            self.assertEqual(hashlib.sha256(p.read_bytes()).hexdigest(), capture['provenance']['fixtures'][p.name])
        self.assertEqual(outcomes, dict(Success=132, Cancel=1))
        for name, digest in capture['provenance']['sources'].items():
            data = (HERE / name).read_bytes()
            if hashlib.sha256(data).hexdigest() != digest and name in ('rhino_worker.py', 'structural_sdk.py'):
                data = gzip.decompress((HERE / 'observations/source_snapshots' / (digest + '.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(), digest, name)

from copy import deepcopy
import json
import math
import unittest
from .report_surface_apex_tangents import ROOT, native, summarize, continuity
from .surface_apex_tangent_cases import request, angle_request, threshold_request
from .surface_apex_command_probe import run, validate
from .client import compare_responses, OracleProtocolError


class SurfaceApexTangentsTests(unittest.TestCase):
    def test_python_api_compares_command_states_and_validates_native_completion(self):
        rows,_,_=native()
        q=request();q['operations']=q['operations'][:1]
        op=q['operations'][0];value=deepcopy(rows[op['id']])
        response=lambda engine,v:dict(engine=engine,engine_version='comparison-test',protocol_version=1,
            iterations=1,results=[dict(id=op['id'],elapsed_ns=0,value=v)])
        local={k:deepcopy(value[k]) for k in ['after','undo','redo','document_tolerance']}
        actual=response('viboceros',local);expected=response('rhino',value)
        self.assertTrue(compare_responses(actual,expected,1e-6,0.,request=q).passed)
        self.assertFalse(compare_responses(actual,expected,1e-6,0.).passed)
        changed=deepcopy(actual)
        changed['results'][0]['value']['redo']['source_unchanged']=False
        self.assertFalse(compare_responses(changed,expected,1e-6,0.,request=q).passed)
        for field in ['events','extra','source_type']:
            changed=deepcopy(expected)
            v=changed['results'][0]['value']
            if field=='events':v[field]=[]
            elif field=='extra':v[field]=True
            else:v['after']['source_present']=1
            with self.subTest(field=field),self.assertRaises(OracleProtocolError):
                compare_responses(actual,changed,1e-6,0.,request=q)

    def test_optional_split_option_is_strict_before_host_access(self):
        queries = [request(), angle_request(), threshold_request()]
        self.assertEqual([len(q['operations']) for q in queries], [128,24,16])
        for q in queries:
            for op in q['operations']:
                validate(op)
        for value in [None,0,1,'Yes',[],{}]:
            op = deepcopy(queries[0]['operations'][0])
            op['split_at_tangents'] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                run(op, {}, 1, queries[0]['tolerance'])

    def test_native_counts_and_volumes_match_independent_profile_properties(self):
        expected, _, _ = native()
        for op in request()['operations']:
            label = op['id'].split('-')[-2]
            walls = (5 if label == 'capsule_seam' else 4) if op['split_at_tangents'] else 1
            if label in ['circle','circle_speed']:
                walls = 1
            if label == 'corners':
                walls = 4
            hole = label == 'hole'
            counts = [walls+int(op['solid'])+int(hole)] if op['solid'] else [walls,1] if hole else [walls]
            products = expected[op['id']]['after']['breps']
            self.assertEqual([b['face_count'] for b in products], counts, op['id'])
            if op['solid']:
                volume = 2.*math.pi if label.startswith('circle') else 16. if label == 'corners' else 16.+(1.82 if hole else 2.)*math.pi
                self.assertLess(abs(products[0]['volume']-volume), 1e-6, op['id'])
        for query, degrees in [(angle_request(),[0.,.05,.5,1.,1.01,2.]),
                               (threshold_request(),[.099,.1,.101,.25])]:
            for op in query['operations']:
                if op['solid']:
                    index = int(op['id'].split('-')[-2])
                    volume = 16.+2.*math.pi+16./3.*math.tan(math.radians(degrees[index]))
                    self.assertLess(abs(expected[op['id']]['after']['breps'][0]['volume']-volume), 1e-6, op['id'])

    def test_report_rebuild_checks_coverage_topology_metrics_and_history(self):
        record = json.loads((ROOT/'docs/surface-apex-tangents-qualification.json').read_text())
        log = lambda rows, prefix: '\n'.join(prefix+' '+json.dumps(r) for r in rows)
        command = log(record['command'],'SURFACE_APEX_TANGENTS_COMMAND')
        app = log(record['app'],'SURFACE_APEX_TANGENTS_APP')
        kernel = record['kernel_continuity']['observations']
        api=record['api']
        self.assertEqual(summarize(command,app,kernel,api),record)
        with self.assertRaises(ValueError):
            summarize(command,log(record['app'][:-1],'SURFACE_APEX_TANGENTS_APP'),kernel,api)
        with self.assertRaises(ValueError):
            summarize(command,app+'\n'+app,kernel,api)
        for phase, field in [('after','face_count'),('after','bounds'),('after','source_unchanged'),
                             ('undo','source_present'),('redo','maximum_error')]:
            changed = deepcopy(record['app'])
            r = changed[0][phase]
            if field == 'face_count':
                r['value']['breps'][0][field] += 1
            elif field == 'bounds':
                r['value']['breps'][0][field][0][0] += .01
            elif field == 'maximum_error':
                r[field] += .01
            else:
                r['value'][field] = not r['value'][field]
            with self.subTest(phase=phase,field=field), self.assertRaises(ValueError):
                summarize(command,log(changed,'SURFACE_APEX_TANGENTS_APP'),kernel,api)
        changed=deepcopy(api);changed[0]['operations'][0]['comparison']['differences']=['changed product']
        with self.assertRaises(ValueError):summarize(command,app,kernel,changed)

    def test_analytic_continuity_proof_rejects_altered_classification_and_inputs(self):
        record = json.loads((ROOT/'docs/surface-apex-tangents-qualification.json').read_text())
        observations = record['kernel_continuity']['observations']
        self.assertEqual(continuity(observations),record['kernel_continuity'])
        for field in ['case','passed','expected','geometric_c2','classification']:
            changed = deepcopy(observations)
            row = changed['patched'][0]
            row[field] = 99 if field in ['case','classification'] else not row[field]
            with self.subTest(field=field), self.assertRaises(ValueError):
                continuity(changed)


if __name__ == '__main__':
    unittest.main()

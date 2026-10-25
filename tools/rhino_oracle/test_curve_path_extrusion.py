"""Native source provenance, strict comparisons, and near-threshold coverage."""
import copy
import json
import unittest
from pathlib import Path
from .client import OracleProtocolError, compare_responses
from .curve_path_comparison import canonical
from .curve_path_extrusion_cases import request,continuity_request
from .curve_path_extrusion_probe import validate
from .report_curve_path_extrusion import native,measurements,summarize,ROOT

HERE=Path(__file__).resolve().parent


class CurvePathExtrusionTests(unittest.TestCase):
    def test_native_sources_and_all_recipes_are_complete(self):
        rows,operations,paths,versions=native()
        self.assertEqual(len(rows),272)
        self.assertTrue(all(versions))
        self.assertTrue(any('source_snapshots' in p for p in paths))
        for op in operations.values():validate(op)
        failures=[v for v in rows.values()if not v['succeeded']]
        self.assertEqual(len(failures),8)
        self.assertEqual(sum(not any(r['source']==0 for r in v['after'])for v in failures),4)

    def test_both_angular_policies_are_independently_visible(self):
        rows,_,_,_=native()
        for axis in ['profile','path']:
            prefix='curve-path-continuity-{}-0.101-'.format(axis)
            self.assertEqual(next(r['face_count']for r in rows[prefix+'010']['after']if r['source'] is None),1)
            self.assertEqual(next(r['face_count']for r in rows[prefix+'100']['after']if r['source'] is None),2)
            self.assertEqual(next(r['face_count']for r in rows[prefix+'110']['after']if r['source'] is None),2)
        for key in ['curve-path-continuity-path-curvature-open-010','curve-path-continuity-path-curvature-circle-010']:
            self.assertEqual(next(r['face_count']for r in rows[key]['after']if r['source'] is None),2)

    def test_request_rejects_nonboolean_and_malformed_curve_inputs(self):
        valid=request()['operations'][0]
        for key in ['crease_splitting','split_at_tangents','solid','delete_input','preselect']:
            invalid=copy.deepcopy(valid);invalid[key]=1
            with self.assertRaises(ValueError):validate(invalid)
        for name,value in [('degree',True),('degree',0),('knots',[0.,float('nan')]),('control_points',[])]:
            invalid=copy.deepcopy(valid);invalid['path'][name]=value
            with self.assertRaises(ValueError):validate(invalid)
        for value in [0.,float('inf')]:
            invalid=copy.deepcopy(valid);invalid['profile']['control_points'][0]['weight']=value
            with self.assertRaises(ValueError):validate(invalid)

    def test_projection_keeps_model_data_and_only_removes_native_trace(self):
        q=request();data=json.loads((HERE/'observations/curve_path_extrusion.json').read_text())
        op=q['operations'][0];value=data['results'][0]['value']
        projected=canonical(op,value,True)
        self.assertNotIn('selected',projected['after'][0])
        self.assertEqual(canonical(op,projected,False),projected)
        invalid=copy.deepcopy(value);invalid['events']*=2
        with self.assertRaises(OracleProtocolError):canonical(op,invalid,True)
        for field,value in [('face_count',True),('is_solid',1),('volume',3.),('bounds',[[0.,0.,0.],[float('nan'),1.,1.]])]:
            invalid=copy.deepcopy(data['results'][0]['value']);invalid['after'][-1][field]=value
            with self.assertRaises(OracleProtocolError):canonical(op,invalid,True)
        invalid=copy.deepcopy(projected);invalid['after'][0]['extra']=0
        with self.assertRaises(OracleProtocolError):canonical(op,invalid,False)

    def test_public_comparison_detects_geometry_and_source_retention_changes(self):
        query=request();native_response=json.loads((HERE/'observations/curve_path_extrusion.json').read_text())
        local=copy.deepcopy(native_response);local['engine']='viboceros';local.pop('provenance')
        for op,row in zip(query['operations'],local['results']):row['value']=canonical(op,row['value'],True)
        self.assertTrue(compare_responses(local,native_response,absolute_epsilon=1e-6,relative_epsilon=0.,request=query).passed)
        local['results'][0]['value']['after'][-1]['face_count']+=1
        self.assertFalse(compare_responses(local,native_response,absolute_epsilon=1e-6,relative_epsilon=0.,request=query).passed)
        for op,row in zip(query['operations'],local['results']):
            if not row['value']['succeeded']:
                row['value']['after']=[{'source':0,'unchanged':True},{'source':1,'unchanged':True}]
        report=compare_responses(local,native_response,absolute_epsilon=1e-6,relative_epsilon=0.,request=query)
        self.assertEqual(sum(not r.passed for r in report.operations),5)

    def test_measurements_reject_incomplete_or_fabricated_logs(self):
        rows,_,_,_=native()
        with self.assertRaises(ValueError):measurements('', 'CURVE_PATH_COMMAND', rows)
        with self.assertRaises(ValueError):measurements('CURVE_PATH_COMMAND '+json.dumps({'id':next(iter(rows))}), 'CURVE_PATH_COMMAND', rows)

    def test_qualification_rebuild_and_corrupted_geometry_detection(self):
        record=json.loads((ROOT/'docs/curve-path-extrusion-qualification.json').read_text())
        def log(rows,prefix):
            failure='CURVE_PATH_FAILURE' if prefix=='CURVE_PATH_COMMAND' else prefix+'_FAILURE'
            return '\n'.join([prefix+' '+json.dumps(r)for r in rows['successful']]+
                             [failure+' '+json.dumps(r)for r in rows['failed']])
        command=log(record['command'],'CURVE_PATH_COMMAND')
        app=log(record['app'],'CURVE_PATH_APP')
        portable=log(record['portable'],'CURVE_PATH_COMMAND') if record.get('portable') else None
        self.assertEqual(summarize(command,app,record.get('python_api'),portable),record)
        for change in ['missing','duplicate','bounds','undo','failure']:
            changed=copy.deepcopy(record['app'])
            if change=='missing':changed['successful'].pop()
            elif change=='duplicate':changed['successful'].append(changed['successful'][0])
            elif change=='bounds':changed['successful'][0]['value']['after'][-1]['bounds'][0][0]+=.01
            elif change=='undo':changed['successful'][0]['value']['undo'][0]['unchanged']=False
            else:changed['failed'][0]['actual']['after'].pop(0)
            with self.subTest(change=change),self.assertRaises(ValueError):
                summarize(command,log(changed,'CURVE_PATH_APP'),record.get('python_api'),portable)


if __name__=='__main__':unittest.main()

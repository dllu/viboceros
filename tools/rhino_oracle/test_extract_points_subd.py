"""Whole-SubD receipts and measured geometry are independently checked."""
from copy import deepcopy
import json
import unittest
from unittest.mock import patch
from .report_extract_points_subd import ROOT,native_subds,summarize


class WholeSubDExtractionTests(unittest.TestCase):
    def test_native_vertex_and_appearance_witnesses_reject_changed_data(self):
        version,rows,paths=native_subds()
        self.assertEqual(len(rows),56)
        self.assertEqual(sum(r['_path']=='picked' for r in rows.values()),28)
        identifier='final_whole_subd_irregular_smooth_points'
        for field in ['appearance','surface','control','duplicate_id','output','macro','cloud']:
            changed=deepcopy(rows);row=changed[identifier]
            if field=='appearance':row['subd_witness']['control_net']=True
            elif field in ['surface','control']:
                # Both surface and control receipts are structurally checked.
                row['subd_witness']['sources'][0]['vertices'][0][field][0]=float('nan')
            elif field=='duplicate_id':
                vertices=row['subd_witness']['sources'][0]['vertices'];vertices[1]['id']=vertices[0]['id']
            elif field=='macro':row['macro']='_-ExtractPt'
            elif field=='cloud':row['after'][-1]['cloud']=True
            else:row['after'][-1]['points'][0][0]+=1.
            with self.subTest(field=field),patch('tools.rhino_oracle.report_extract_points_subd.native',side_effect=[(version,changed,paths),(version,{},set())]),self.assertRaises(ValueError):
                native_subds()
        changed=deepcopy(rows)
        changed['picked_whole_subd_irregular_smooth_points']['getter']['selection']=''
        with patch('tools.rhino_oracle.report_extract_points_subd.native',side_effect=[(version,changed,paths),(version,{},set())]),self.assertRaises(ValueError):
            native_subds()

    def test_reports_recompute_and_reject_missing_duplicate_or_forged_measurements(self):
        record=json.loads((ROOT/'docs/extract-points-subd-qualification.json').read_text())
        log=lambda rows,prefix:'\n'.join(prefix+' '+json.dumps({k:v for k,v in row.items() if k!='maximum_deviation'}) for row in rows)
        commands=log(record['commands'],'EXTRACT_POINTS_SUBD_COMMAND')
        getters=log(record['getters'],'EXTRACT_POINTS_SUBD_GETTER')
        self.assertEqual(summarize(commands,getters),record)
        with self.assertRaises(ValueError):summarize(commands,log(record['getters'][:-1],'EXTRACT_POINTS_SUBD_GETTER'))
        with self.assertRaises(ValueError):summarize(commands+'\n'+commands,getters)
        for field in ['point','attributes','selection','storage']:
            changed=deepcopy(record['commands'])
            if field=='point':changed[0]['after'][-1]['points'][0][0]+=1.
            elif field=='attributes':changed[0]['after'][-1]['name']='forged'
            elif field=='selection':changed[0]['redo'][-1]['selected']=not changed[0]['redo'][-1]['selected']
            else:changed[0]['source_storage_unchanged']=False
            with self.subTest(field=field),self.assertRaises(ValueError):summarize(log(changed,'EXTRACT_POINTS_SUBD_COMMAND'),getters)

"""Native temporary references and strict measured-state coverage."""
from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import json
import unittest
from unittest.mock import patch
from .report_extract_point_ranges import ROOT,native_ranges,summarize

class ExtractPointRangeTests(unittest.TestCase):
    def test_mixed_input_receipts_reject_changed_order_and_missing_macro_picks(self):
        version,rows,paths=native_ranges()
        identifier=next(k for k,r in rows.items() if r['input'].get('input_order') and
            len(r['input']['input_order'])>2)
        for field in ['inputs','script','world_coordinates']:
            changed=deepcopy(rows)
            trace=changed[identifier]['getter']
            if field=='inputs':trace[field]=list(reversed(trace[field]))
            elif field=='script':trace[field]=trace[field].replace(' _SelID ',' _MissingID ',1)
            else:trace[field]=False
            with self.subTest(field=field),patch('tools.rhino_oracle.report_extract_point_ranges.native',return_value=(version,changed,paths)),self.assertRaises(ValueError):
                native_ranges()

    def test_edge_witness_rejects_construction_plane_input_and_wrong_endpoints(self):
        version,rows,paths=native_ranges()
        identifier='range_form_brep_cylinder_forward_points'
        for field in ['world_coordinates','endpoint','curve_type']:
            changed=deepcopy(rows)
            trace=changed[identifier]['getter']
            if field=='world_coordinates':trace[field]=False
            elif field=='endpoint':trace['points'][0][1][0]+=1.
            else:trace[field]='NurbsCurve'
            with self.subTest(field=field),patch('tools.rhino_oracle.report_extract_point_ranges.native',return_value=(version,changed,paths)),self.assertRaises(ValueError):
                native_ranges()

    def test_native_range_receipts_source_archives_and_parameter_inputs(self):
        _,rows,_=native_ranges()
        self.assertEqual(len(rows),76)
        for row in rows.values():
            self.assertTrue(row['success'])
            self.assertFalse(any(o['selected'] for o in row['redo'] if o['source'] is None))

    def test_recomputed_reports_reject_missing_duplicate_or_forged_measurements(self):
        record=json.loads((ROOT/'docs/extract-point-ranges-qualification.json').read_text())
        log=lambda rows,prefix:'\n'.join(prefix+' '+json.dumps({k:v for k,v in row.items() if k!='maximum_deviation'}) for row in rows)
        commands=log(record['commands'],'EXTRACT_POINTS_RANGE_COMMAND')
        getters=log(record['getters'],'EXTRACT_POINTS_RANGE_GETTER')
        self.assertEqual(reconstruct_measurement(record, summarize, commands,getters),record)
        with self.assertRaises(ValueError):summarize(commands,log(record['getters'][:-1],'EXTRACT_POINTS_RANGE_GETTER'))
        with self.assertRaises(ValueError):summarize(commands+'\n'+commands,getters)
        for field in ['geometry','attributes','selection','storage']:
            forged=deepcopy(record['commands'])
            if field=='geometry':forged[0]['after'][-1]['points'][0][0]+=1.
            elif field=='attributes':forged[0]['after'][-1]['layer']='wrong'
            elif field=='selection':forged[0]['redo'][-1]['selected']=True
            else:forged[0]['source_storage_unchanged']=False
            with self.subTest(field=field),self.assertRaises(ValueError):summarize(log(forged,'EXTRACT_POINTS_RANGE_COMMAND'),getters)

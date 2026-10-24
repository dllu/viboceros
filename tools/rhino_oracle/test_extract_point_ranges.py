"""Native temporary references and strict measured-state coverage."""
from copy import deepcopy
import json
import unittest
from .report_extract_point_ranges import ROOT,native_ranges,summarize

class ExtractPointRangeTests(unittest.TestCase):
    def test_native_range_receipts_source_archives_and_parameter_inputs(self):
        _,rows,_=native_ranges()
        self.assertEqual(len(rows),26)
        for row in rows.values():
            self.assertTrue(row['success'])
            self.assertFalse(any(o['selected'] for o in row['redo'] if o['source'] is None))

    def test_recomputed_reports_reject_missing_duplicate_or_forged_measurements(self):
        record=json.loads((ROOT/'docs/extract-point-ranges-qualification.json').read_text())
        log=lambda rows,prefix:'\n'.join(prefix+' '+json.dumps({k:v for k,v in row.items() if k!='maximum_deviation'}) for row in rows)
        commands=log(record['commands'],'EXTRACT_POINTS_RANGE_COMMAND')
        getters=log(record['getters'],'EXTRACT_POINTS_RANGE_GETTER')
        self.assertEqual(summarize(commands,getters),record)
        with self.assertRaises(ValueError):summarize(commands,log(record['getters'][:-1],'EXTRACT_POINTS_RANGE_GETTER'))
        with self.assertRaises(ValueError):summarize(commands+'\n'+commands,getters)
        for field in ['geometry','attributes','selection','storage']:
            forged=deepcopy(record['commands'])
            if field=='geometry':forged[0]['after'][-1]['points'][0][0]+=1.
            elif field=='attributes':forged[0]['after'][-1]['layer']='wrong'
            elif field=='selection':forged[0]['redo'][-1]['selected']=True
            else:forged[0]['source_storage_unchanged']=False
            with self.subTest(field=field),self.assertRaises(ValueError):summarize(log(forged,'EXTRACT_POINTS_RANGE_COMMAND'),getters)

from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import json
import unittest
from unittest.mock import patch
from .report_subd_display import ROOT,native_display,summarize

class SubDDisplayTests(unittest.TestCase):
    def test_native_appearance_and_preserved_selection_are_verified(self):
        version,rows,paths=native_display()
        self.assertEqual(len(rows),36)
        identifier=next(iter(rows))
        for field in ['initial','result','selection','history']:
            changed=deepcopy(rows);r=changed[identifier]
            if field in ['initial','result']:r['display_toggle'][field]=not r['display_toggle'][field]
            elif field=='selection':r['display_toggle']['after'][0]['selected']=not r['display_toggle']['after'][0]['selected']
            else:r['display_modes'][1]=not r['display_modes'][1]
            with self.subTest(field=field),patch('tools.rhino_oracle.report_subd_display.native',return_value=(version,changed,paths)),self.assertRaises(ValueError):native_display()

    def test_measurement_reports_rebuild_and_reject_forged_or_missing_states(self):
        record=json.loads((ROOT/'docs/subd-display-qualification.json').read_text())
        log=lambda rows,prefix:'\n'.join(prefix+' '+json.dumps({k:v for k,v in r.items() if k!='maximum_deviation'}) for r in rows)
        commands=log(record['commands'],'SUBD_DISPLAY_COMMAND');getters=log(record['getters'],'SUBD_DISPLAY_GETTER')
        self.assertEqual(reconstruct_measurement(record, summarize, commands,getters),record)
        with self.assertRaises(ValueError):summarize(commands,log(record['getters'][:-1],'SUBD_DISPLAY_GETTER'))
        with self.assertRaises(ValueError):summarize(commands+'\n'+commands,getters)
        for field in ['point','mode','preserved']:
            changed=deepcopy(record['commands'])
            if field=='point':changed[0]['after'][-1]['points'][0][0]+=1.
            elif field=='mode':changed[0]['modes'][0]=not changed[0]['modes'][0]
            else:changed[0]['toggle_preserved']=False
            with self.subTest(field=field),self.assertRaises(ValueError):summarize(log(changed,'SUBD_DISPLAY_COMMAND'),getters)

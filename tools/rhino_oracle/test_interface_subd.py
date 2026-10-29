from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import json
import unittest
from unittest.mock import patch
from .report_interface_subd import ROOT,native,summarize
from .references.interface_subd import request

class InterfaceSubDTests(unittest.TestCase):
    def test_native_states_cover_initial_values_and_reject_changed_appearance(self):
        data,rows,paths=native();self.assertEqual(len(rows),64)
        self.assertEqual(sum(len(r['states']) for r in rows.values()),640)
        record=json.loads((ROOT/'docs/interface-subd-qualification.json').read_text())
        self.assertEqual(record['native_version'],data['engine_version'])
        for field in ['appearance','type','initial','view','count']:
            changed=deepcopy(data);states=changed['results'][0]['value']['states']
            if field=='appearance':states[1]['subd_control_net']=False
            elif field=='type':states[1]['subd_control_net']=1
            elif field=='initial':states[0]['grid_snap']=not states[0]['grid_snap']
            elif field=='view':states[1]['active_viewport']=9
            else:states.pop()
            with self.subTest(field=field),patch('tools.rhino_oracle.report_interface_subd.json.loads',side_effect=[request(),changed]),self.assertRaises(ValueError):native()

    def test_reports_require_complete_distinct_and_exact_measurements(self):
        record=json.loads((ROOT/'docs/interface-subd-qualification.json').read_text())
        log=lambda rows,prefix:'\n'.join(prefix+' '+json.dumps(r) for r in rows)
        oracle=log(record['oracle'],'INTERFACE_SUBD_ORACLE');app=log(record['app'],'INTERFACE_SUBD_APP')
        self.assertEqual(reconstruct_measurement(record, summarize, oracle,app),record)
        with self.assertRaises(ValueError):summarize(oracle,log(record['app'][:-1],'INTERFACE_SUBD_APP'))
        with self.assertRaises(ValueError):summarize(oracle+'\n'+oracle,app)
        for field in ['subd_control_net','grid_snap','display_modes','active_viewport','state_count','type']:
            changed=deepcopy(record['oracle']);state=changed[0]['value']['states'][1]
            if field in ['subd_control_net','grid_snap']:state[field]=not state[field]
            elif field=='display_modes':state[field][0]='Rendered'
            elif field=='active_viewport':state[field]=9
            elif field=='state_count':changed[0]['value']['states'].pop()
            else:state['subd_control_net']=int(state['subd_control_net'])
            with self.subTest(field=field),self.assertRaises(ValueError):summarize(log(changed,'INTERFACE_SUBD_ORACLE'),app)

"""Prevent complete source certificates from weakening native output gates."""
import copy
import json
import unittest
from .report_surface_image_auto import PROOF, ROOT, read, data, report, rows, image_summary


class Record(unittest.TestCase):
    def inputs(self):
        receipt=json.loads((PROOF/'receipts.json').read_bytes())
        return data(), read(ROOT/receipt['checks']['portable']['log']).decode(), read(ROOT/receipt['baselines']['portable']).decode()

    def replace(self,log,row,wrong):
        changed=log.replace('SURFACE_IMAGE_FIT '+json.dumps(row,sort_keys=True,separators=(',',':')),
            'SURFACE_IMAGE_FIT '+json.dumps(wrong,sort_keys=True,separators=(',',':')))
        self.assertNotEqual(changed,log)
        return changed

    def test_frozen_record_matches(self):
        self.assertEqual(report(),json.loads((ROOT/'docs/surface-image-auto-validation.json').read_bytes()))

    def test_relaxed_native_gate_or_incomplete_maps_are_rejected(self):
        captures,log,baseline=self.inputs()
        row=next(r for r in rows(log,'SURFACE_IMAGE_FIT') if r['native_parameter_bound'] is not None)
        wrong=copy.deepcopy(row);wrong['native_parameter_bound']=1e-5
        with self.assertRaises(AssertionError):image_summary(captures,self.replace(log,row,wrong),baseline)
        row=next(r for r in rows(log,'SURFACE_IMAGE_FIT') if r['certificate'] is not None and r['certificate']['type']=='separable_uv')
        wrong=copy.deepcopy(row);wrong['certificate']['correspondence'][0][-1]=[.5,1.]
        with self.assertRaises(AssertionError):image_summary(captures,self.replace(log,row,wrong),baseline)

    def test_source_accuracy_cannot_replace_native_agreement_or_omit_rechecks(self):
        captures,log,baseline=self.inputs()
        row=next(r for r in rows(log,'SURFACE_IMAGE_FIT') if r['status']=='native_image_unproved')
        wrong=copy.deepcopy(row);wrong['status']='native_image_match'
        with self.assertRaises(AssertionError):image_summary(captures,self.replace(log,row,wrong),baseline)
        wrong=copy.deepcopy(row);wrong['source_rechecked_bound']=None
        with self.assertRaises(AssertionError):image_summary(captures,self.replace(log,row,wrong),baseline)


if __name__=='__main__':unittest.main()

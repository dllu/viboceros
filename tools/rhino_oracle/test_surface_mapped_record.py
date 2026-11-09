"""The record must preserve full-image and strict native-output gates."""
import copy
import json
import unittest
from .report_surface_mapped import PROOF, ROOT, read, report, summarize, rows


class Record(unittest.TestCase):
    def data(self):
        receipt = json.loads((PROOF/'receipts.json').read_bytes())
        return (json.loads((ROOT/'tools/rhino_oracle/fixtures/surface_fit_achieved.json').read_bytes()),
            json.loads(read(ROOT/'tools/rhino_oracle/observations/surface_fit_achieved.json.gz')),
            read(ROOT/receipt['checks']['portable']['log']).decode())

    def test_frozen_record_matches(self):
        self.assertEqual(report(), json.loads((ROOT/'docs/surface-mapped-validation.json').read_bytes()))

    def test_relaxed_native_gate_or_incomplete_maps_are_rejected(self):
        q, n, log = self.data()
        row = rows(log, 'MAPPED_BASIS')[0]; wrong = copy.deepcopy(row); wrong['native_bound'] = 1e-5
        altered = log.replace('MAPPED_BASIS '+json.dumps(row,separators=(',',':'),sort_keys=True),
            'MAPPED_BASIS '+json.dumps(wrong,separators=(',',':'),sort_keys=True))
        self.assertNotEqual(altered, log)
        with self.assertRaises(AssertionError): summarize(q,n,altered)
        row = next(r for r in rows(log,'MAPPED_FIT') if r.get('mapped_bound') is not None)
        wrong = copy.deepcopy(row); wrong['correspondence'][0][-1] = [.5,1.]
        altered = log.replace('MAPPED_FIT '+json.dumps(row,separators=(',',':'),sort_keys=True),
            'MAPPED_FIT '+json.dumps(wrong,separators=(',',':'),sort_keys=True))
        self.assertNotEqual(altered, log)
        with self.assertRaises(AssertionError): summarize(q,n,altered)

    def test_source_image_tolerance_cannot_be_published_as_native_output_parity(self):
        q, n, log = self.data()
        row = next(r for r in rows(log,'MAPPED_FIT') if r.get('mapped_bound') is not None)
        wrong = copy.deepcopy(row); wrong['native_output_parity'] = True
        altered = log.replace('MAPPED_FIT '+json.dumps(row,separators=(',',':'),sort_keys=True),
            'MAPPED_FIT '+json.dumps(wrong,separators=(',',':'),sort_keys=True))
        self.assertNotEqual(altered, log)
        with self.assertRaises(AssertionError): summarize(q,n,altered)


if __name__ == '__main__': unittest.main()

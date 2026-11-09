"""Prevent omissions and conflation of input bounds with native agreement."""
import json
import unittest
from . import report_surface_fit_adaptive as audit


class SurfaceFitAdaptiveRecordTests(unittest.TestCase):
    def evidence(self):
        receipts=json.loads((audit.PROOF/'receipts.json').read_bytes())
        log=audit.basis.read(audit.ROOT/receipts['checks']['portable']['log']).decode()
        return audit.data(),log

    def test_record_reproduces_original_sources_and_complete_inventory(self):
        record=json.loads((audit.ROOT/'docs/surface-fit-adaptive-validation.json').read_bytes())
        self.assertEqual(audit.report(),record)
        self.assertFalse(record['full_rhino_fit_parity'])
        self.assertEqual(record['native_surfaces']+record['native_null'],112)

    def test_dropped_rows_or_changed_native_classification_fail(self):
        data,log=self.evidence()
        for prefix in ['AUTOMATIC_SURFACE_FIT ','ADAPTIVE_SURFACE_BASIS ','FIT_REPRESENTATION ']:
            line=next(s for s in log.splitlines()if s.startswith(prefix))
            with self.assertRaises(AssertionError):audit.summarize(data,log.replace(line,'',1))
        data[0][1]['results'][0]['value']['surface']=None
        with self.assertRaises(AssertionError):audit.summarize(data,log)

    def test_parameter_and_archive_gates_are_not_relaxed(self):
        data,log=self.evidence()
        for field in ['native_bound','archive_bound','source_bound']:
            lines=log.splitlines()
            index=next(i for i,line in enumerate(lines)if line.startswith('AUTOMATIC_SURFACE_FIT ')and json.loads(line.split(' ',1)[1])['source_bound']is not None)
            row=json.loads(lines[index].split(' ',1)[1]);row[field]=row['limit']+1. if field=='source_bound'else 1e-8
            lines[index]='AUTOMATIC_SURFACE_FIT '+json.dumps(row)
            with self.assertRaises(AssertionError):audit.summarize(data,'\n'.join(lines))
        with self.assertRaises(AssertionError):audit.summarize(data,log.replace('"full_rhino_fit_parity":false','"full_rhino_fit_parity":true'))


if __name__=='__main__':unittest.main()

"""Keep explicit-basis evidence distinct from a complete adaptive Fit claim."""
import json
import unittest
from . import report_surface_fit_basis as audit


class SurfaceFitRecordTests(unittest.TestCase):
    def data(self):
        query = json.loads((audit.ROOT / 'tools/rhino_oracle/fixtures/surface_fit.json').read_bytes())
        native = json.loads(audit.read(audit.ROOT / 'tools/rhino_oracle/observations/surface_fit.json.gz'))
        receipts = json.loads((audit.PROOF / 'receipts.json').read_bytes())
        log = audit.read(audit.ROOT / receipts['checks']['portable']['log']).decode()
        return query, native, log

    def test_record_reproduces_with_original_sources_and_launch_receipts(self):
        expected = json.loads((audit.ROOT / 'docs/surface-fit-basis-validation.json').read_bytes())
        self.assertEqual(audit.report(), expected)
        self.assertFalse(expected['adaptive_basis_qualified'])
        self.assertTrue(expected['captured_bases_supplied_by_test'])
        self.assertEqual(expected['backends']['portable']['native_null'], 13)

    def test_missing_rows_and_native_failures_cannot_be_counted_as_basis_matches(self):
        query, native, log = self.data()
        first = next(line for line in log.splitlines() if line.startswith('SURFACE_FIT_BASIS '))
        with self.assertRaises(AssertionError):
            audit.summarize(query, native, log.replace(first, '', 1))
        native['results'][0]['value']['surface'] = None
        with self.assertRaises(AssertionError):
            audit.summarize(query, native, log)

    def test_error_gate_and_adaptive_qualification_are_not_relaxed(self):
        query, native, log = self.data()
        for key, value in [('maximum_native_bound', 1e-8), ('maximum_archive_bound', 1e-8)]:
            lines = log.splitlines()
            index = next(i for i, line in enumerate(lines) if line.startswith('SURFACE_FIT_BASIS '))
            row = json.loads(lines[index].split(' ', 1)[1]); row[key] = value
            lines[index] = 'SURFACE_FIT_BASIS ' + json.dumps(row)
            with self.assertRaises(AssertionError):
                audit.summarize(query, native, '\n'.join(lines))
        with self.assertRaises(AssertionError):
            audit.summarize(query, native, log.replace('"adaptive_fit_qualified":false', '"adaptive_fit_qualified":true'))


if __name__ == '__main__':
    unittest.main()

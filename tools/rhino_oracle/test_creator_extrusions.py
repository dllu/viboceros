"""Fixed native creator provenance and fail-closed qualification checks."""
from tools.verify_provenance import reconstruct_measurement, verify_measurement
import copy
import gzip
import hashlib
import json
import unittest
from . import creator_extrusions_probe as probe
from .report_creator_extrusions import ROOT,HERE,observations,summarize
from .structural_sdk import validate_operations


class CreatorExtrusionTests(unittest.TestCase):
    def test_fixed_recipes_and_archived_producers(self):
        query=json.loads((HERE/'fixtures/creator_extrusions.json').read_text())
        self.assertEqual(query,probe.request());validate_operations(query)
        native=json.loads((HERE/'observations/creator_extrusions.json').read_text())
        self.assertEqual(len(observations()[0]),42)
        for row in native['results']:self.assertEqual(row['value']['input'],probe.CASES[row['id']])
        for name in ['creator_extrusions','creator_extrusions_macro_diagnostic']:
            captured=json.loads((HERE/'observations'/(name+'.json')).read_text())
            provenance=captured['provenance']
            self.assertEqual(provenance['request_sha256'],hashlib.sha256((HERE/'fixtures'/(name+'.json')).read_bytes()).hexdigest())
            for path,sha in provenance['sources'].items():
                data=(HERE/path).read_bytes()
                if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha,path)
        diagnostic=json.loads((HERE/'observations/creator_extrusions_macro_diagnostic.json').read_text())
        self.assertIs(diagnostic['excluded_from_qualification'],True);self.assertIn('Unknown command: _Solid=_No',diagnostic['error'])

    def lines(self):
        report=json.loads((ROOT/'docs/creator-extrusion-qualification.json').read_text())
        return report,['CREATOR_EXTRUSIONS '+json.dumps(r)for r in report['cases']]

    def test_complete_report_and_historical_source_hashes(self):
        report,lines=self.lines();self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(lines)),report)
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(reversed(lines))),report)
        self.assertEqual((report['creator_cases'],report['typed_extrusion_creators'],report['tosubd_comparisons']),(42,39,40))
        self.assertIs(report['full_native_parity'],False);self.assertIs(report['performance_qualified'],False)
        self.assertGreater(verify_measurement(report), 0)

    def test_default_build_matches_the_same_native_creator_workflows(self):
        report=json.loads((ROOT/'docs/creator-extrusion-portable-qualification.json').read_text())
        lines=['CREATOR_EXTRUSIONS '+json.dumps(row)for row in report['cases']]
        self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(lines),backend='portable'),report)
        self.assertEqual(report['modeling_backend'],'portable')
        self.assertEqual((report['creator_cases'],report['typed_extrusion_creators'],report['tosubd_comparisons']),(42,39,40))
        self.assertGreater(verify_measurement(report), 0)

    def test_incomplete_duplicate_and_invalid_certificates_fail(self):
        report,lines=self.lines()
        for bad in [lines[:-1],lines+lines[:1]]:
            with self.assertRaises(ValueError):summarize('\n'.join(bad))
        row=report['cases'][0];original='CREATOR_EXTRUSIONS '+json.dumps(row)
        for path,bad in [(['typed_extrusion'],1),(['history'],False),(['roundtrip'],False),(['maximum_surface_control_error'],float('nan')),(['maximum_bounds_error'],1.1e-6),(['conversion','qualified'],False),(['conversion','faces'],True),(['conversion','maximum_sampled_surface_error'],1.1e-6)]:
            changed=copy.deepcopy(row);target=changed
            for key in path[:-1]:target=target[key]
            target[path[-1]]=bad
            with self.assertRaises(ValueError,msg=path):summarize('\n'.join('CREATOR_EXTRUSIONS '+json.dumps(changed)if s==original else s for s in lines))

    def test_existing_direction_api_keeps_all_thirty_native_cases(self):
        from tools.verify_provenance import verify
        record=json.loads((ROOT/'docs/creator-extrusion-direction-regression.json').read_text())
        self.assertEqual({name:r['cases']for name,r in record['reports'].items()},{'curve_extrusion_directions':27,'curve_native_extrusion':3})
        for report in record['reports'].values():
            self.assertIs(report['passed'],True);self.assertEqual(report['matched'],report['cases']);self.assertEqual(report['native_failed'],0)
        self.assertGreater(verify(record),0)

    def test_arbitrary_creator_recipes_are_rejected(self):
        for edit in [dict(case='arbitrary'),dict(case=[]),dict(extra=True),dict(id='wrong')]:
            bad=probe.request();bad['operations'][0].update(edit)
            with self.assertRaises(ValueError):validate_operations(bad)


if __name__=='__main__':unittest.main()

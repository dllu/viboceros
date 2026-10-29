"""Frozen vertex-seam producers, standalone staging and strict qualification."""
from tools.verify_provenance import reconstruct_measurement
import base64
import copy
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .component_seams_subd_cases import ALL_CASES,request,validate
from .report_vertex_seams import FAMILIES,HERE,ROOT,observations,summarize
from .structural_sdk import stage_helpers,validate_operations


class VertexSeamTests(unittest.TestCase):
    def test_fixed_producers_archives_and_exact_saved_sources(self):
        for family,count in FAMILIES.items():
            path=HERE/'fixtures'/(family+'.json');query=json.loads(path.read_text())
            self.assertEqual(query,request(followup=family.endswith('_followup'),grips=family.endswith('_grips')))
            validate_operations(query)
            capture=json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(len(capture['results']),count)
            self.assertEqual(capture['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in capture['provenance']['sources'].items():
                data=(HERE/name).read_bytes()
                if hashlib.sha256(data).hexdigest()!=digest:data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)
            for row in capture['results']:
                self.assertEqual(row['value']['input'],ALL_CASES[row['id']])
                self.assertEqual((HERE/'fixtures'/family/(row['id']+'.3dm')).read_bytes(),base64.b64decode(row['value']['source_3dm'],validate=True))

    def test_strict_allowlist_and_standalone_helpers(self):
        for change in [dict(id='other'),dict(case=[]),dict(case='arbitrary'),dict(extra=True)]:
            operation=copy.deepcopy(request()['operations'][0]);operation.update(change)
            with self.assertRaises(ValueError):validate(operation)
            with self.assertRaises(ValueError):validate_operations(dict(request(),operations=[operation]))
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(request(),HERE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import component_seams_subd_probe',directory],check=True,capture_output=True)

    def report_lines(self):
        report=json.loads((ROOT/'docs/vertex-seams-qualification.json').read_text())
        lines=['VERTEX_SEAM_COMMAND '+json.dumps(r)for r in report['cases']]
        lines+=['VERTEX_SEAM_GETTER '+json.dumps(r)for r in report['getters']]
        lines+=['SUBD_SURFACE_GRIP '+json.dumps(r)for r in report['surface_grips']]
        return report,lines

    def test_historical_report_reconstructs_and_keeps_edge_diagnostics_separate(self):
        expected,engine=observations();self.assertEqual(len(expected),46);self.assertEqual(engine,'8.32.26160.13001')
        report,lines=self.report_lines();self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(reversed(lines))),report)
        self.assertEqual(report['getter_cases'],80);self.assertEqual(report['edge_diagnostic_cases'],26)
        self.assertEqual(report['surface_grip_cases'],9);self.assertEqual(report['surface_grip_native_matches'],6)
        self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])

    def test_incomplete_or_forged_evidence_is_rejected(self):
        report,lines=self.report_lines()
        for text in ['\n'.join(lines[:-1]),'\n'.join(lines+lines[-1:])]:
            with self.assertRaises(ValueError):summarize(text)
        for key,bad in [('id','unknown'),('maximum_error',True),('maximum_error',float('nan')),('maximum_error',1.1e-6),('identity',False),('attributes',1),('history',False),('processed_objects',True),('processed_objects',99)]:
            row=dict(report['cases'][0],**{key:bad})
            with self.assertRaises(ValueError):summarize('\n'.join(['VERTEX_SEAM_COMMAND '+json.dumps(row)]+lines[1:]))
        original='VERTEX_SEAM_GETTER '+json.dumps(report['getters'][0])
        for key,bad in [('phase','unknown'),('selection',False),('maximum_error',float('inf'))]:
            changed='VERTEX_SEAM_GETTER '+json.dumps(dict(report['getters'][0],**{key:bad}))
            with self.assertRaises(ValueError):summarize('\n'.join(changed if line==original else line for line in lines))


if __name__=='__main__':unittest.main()

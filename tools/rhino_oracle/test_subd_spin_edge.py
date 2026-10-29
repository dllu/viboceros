"""Immutable edge-spin producers and strict command/getter qualification."""
from tools.verify_provenance import reconstruct_measurement
import base64
import gzip
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from .subd_spin_edge_cases import ALL_CASES,request
from .report_subd_spin_edge import FAMILIES,HERE,ROOT,summarize,observations
from .structural_sdk import stage_helpers,validate_operations


class SubDSpinEdgeTests(unittest.TestCase):
    def test_fixed_sources_producer_archives_and_exact_before_files(self):
        flags={'subd_spin_edge_discovery':'discovery','subd_spin_edge_live':'live','subd_spin_edge_live_v2':'live_v2','subd_spin_edge_live_v3':'live_v3','subd_spin_edge_holdouts':'holdouts','subd_spin_edge_live_v5':'live_v5'}
        for family,count in FAMILIES.items():
            path=HERE/'fixtures'/(family+'.json');query=json.loads(path.read_text())
            self.assertEqual(query,request(**({flags[family]:True}if family in flags else{})));validate_operations(query)
            capture=json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(len(capture['results']),count)
            self.assertEqual(capture['provenance']['request_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            for name,digest in capture['provenance']['sources'].items():
                data=(HERE/name).read_bytes()
                if hashlib.sha256(data).hexdigest()!=digest:data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)
            for row in capture['results']:
                value=row.get('value')
                if value is None:self.assertTrue(row.get('error'));continue
                self.assertEqual(value['input'],ALL_CASES[row['id']])
                self.assertEqual((HERE/'fixtures'/family/(row['id']+'.3dm')).read_bytes(),base64.b64decode(value['source_3dm'],validate=True))

        timeout=json.loads((HERE/'observations/subd_spin_edge_live_v4_timeout.json').read_text())
        self.assertEqual(timeout['status'],'owned_runner_timeout');self.assertEqual(timeout['qualified_workflows'],0)
        query=HERE/'fixtures/subd_spin_edge_live_v4.json'
        self.assertEqual(json.loads(query.read_text()),request(live_v4=True));self.assertEqual(hashlib.sha256(query.read_bytes()).hexdigest(),timeout['request_sha256'])
        self.assertEqual(hashlib.sha256((HERE/'observations/subd_spin_edge_live_v4_timeout.log').read_bytes()).hexdigest(),timeout['log_sha256'])
        for name,digest in timeout['sources'].items():
            data=(HERE/name).read_bytes()
            if hashlib.sha256(data).hexdigest()!=digest:data=gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())
            self.assertEqual(hashlib.sha256(data).hexdigest(),digest,name)

    def test_strict_allowlist_and_standalone_live_helpers(self):
        query=request(live_v3=True);bad=dict(query['operations'][0],case='arbitrary')
        with self.assertRaises(ValueError):validate_operations(dict(query,operations=[bad]))
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(query,HERE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import subd_spin_edge_probe',directory],check=True,capture_output=True)

    def report_lines(self):
        report=json.loads((ROOT/'docs/subd-spin-edge-qualification.json').read_text())
        return report,(['SUBD_SPIN_COMMAND '+json.dumps(r)for r in report['cases']]
                       +['SUBD_SPIN_GETTER '+json.dumps(r)for r in report['getters']]
                       +['SUBD_SPIN_DISCOVERY '+json.dumps(r)for r in report['geometry']]
                       +['SUBD_SPIN_UNDO_GAP '+json.dumps(r)for r in report['cross_object_undo']])

    def test_report_reconstructs_with_invalid_native_results_kept_separate(self):
        report,lines=self.report_lines();self.assertEqual(reconstruct_measurement(report, summarize, '\n'.join(reversed(lines))),report)
        self.assertEqual(report['geometry_cases'],17);self.assertEqual(report['native_invalid_output_cases'],4)
        self.assertEqual(report['staged_command_cases'],report['getter_cases']);self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])
        expected,core,excluded,_=observations();self.assertTrue(excluded);self.assertEqual(len(core),17);self.assertEqual(len(expected),report['getter_cases'])

    def test_missing_duplicate_nonfinite_and_forged_evidence_fail(self):
        report,lines=self.report_lines()
        for text in ['\n'.join(lines[:-1]),'\n'.join(lines+lines[-1:])]:
            with self.assertRaises(ValueError):summarize(text)
        for kind,key,bad in [('cases','id','unrelated'),('cases','maximum_error',True),('cases','history',1),('cases','processed_objects',True),('getters','metadata',False),('getters','selection',False),('getters','maximum_error',float('nan')),('geometry','valid_output',False),('geometry','repaired',True)]:
            prefix={'cases':'SUBD_SPIN_COMMAND','getters':'SUBD_SPIN_GETTER','geometry':'SUBD_SPIN_DISCOVERY'}[kind]
            original=prefix+' '+json.dumps(report[kind][0]);changed=prefix+' '+json.dumps(dict(report[kind][0],**{key:bad}))
            with self.assertRaises(ValueError):summarize('\n'.join(changed if line==original else line for line in lines))


if __name__=='__main__':unittest.main()

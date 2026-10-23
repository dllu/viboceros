"""Immutable edge observations and strict replay qualification."""
import base64
import copy
import gzip
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from .component_seams_subd_cases import ALL_CASES,request
from .report_edge_seams import FAMILIES,DIAGNOSTIC,HERE,ROOT,observations,summarize
from .structural_sdk import stage_helpers,validate_operations


class EdgeSeamTests(unittest.TestCase):
    def test_frozen_recipes_archives_and_saved_before_geometry(self):
        flags={'component_seams_subd_edges':'edges','component_seams_subd_edge_holdouts':'edge_holdouts',
               'component_seams_subd_edge_contacts':'edge_contacts','component_seams_subd_edge_tolerances_v2':'edge_tolerances',
               DIAGNOSTIC:'edge_tolerances'}
        for family,count in [*FAMILIES.items(),(DIAGNOSTIC,7)]:
            path=HERE/'fixtures'/(family+'.json');query=json.loads(path.read_text())
            self.assertEqual(query,request(**({flags[family]:True}if family in flags else {})))
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
                if family==DIAGNOSTIC:self.assertTrue(all(not o['selected_edges']for o in row['value']['before']if 'subd'in o))

    def test_standalone_helpers_include_edge_recipes(self):
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(request(edges=True),HERE,Path(directory))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import component_seams_subd_probe;import component_edge_seams_cases',directory],check=True,capture_output=True)

    def report_lines(self):
        report=json.loads((ROOT/'docs/edge-seams-qualification.json').read_text())
        lines=['SUBD_EDGE_GEOMETRY '+json.dumps(r)for r in report['geometry']]
        lines+=['EDGE_SEAM_COMMAND '+json.dumps(r)for r in report['cases']]
        lines+=['EDGE_SEAM_GETTER '+json.dumps(r)for r in report['getters']]
        return report,lines

    def test_report_reconstructs_and_retains_explicit_numbering_limits(self):
        scenes,engine=observations();self.assertEqual(len(scenes),86);self.assertEqual(engine,'8.32.26160.13001')
        report,lines=self.report_lines();self.assertEqual(summarize('\n'.join(reversed(lines))),report)
        self.assertEqual(report['geometry_source_cases'],94);self.assertEqual(report['getter_cases'],132)
        self.assertLess(report['raw_control_ordinal_matches'],94)
        self.assertIn(DIAGNOSTIC,report['excluded_diagnostics']);self.assertFalse(report['full_native_parity']);self.assertFalse(report['performance_qualified'])

    def test_missing_duplicates_and_forged_evidence_fail(self):
        report,lines=self.report_lines()
        for text in ['\n'.join(lines[:-1]),'\n'.join(lines+lines[-1:])]:
            with self.assertRaises(ValueError):summarize(text)
        for kind,key,bad in [('geometry','id','unknown'),('geometry','source',True),('geometry','changed_edges',True),('geometry','qualified',False),('geometry','raw_control_ordinals_match',1),('geometry','edge_ordinals_match',1),('geometry','maximum_error',float('nan')),('cases','identity',False),('cases','processed_objects',True),('cases','maximum_error',True),('getters','phase','unknown'),('getters','selection',1),('getters','maximum_error',1.1e-6)]:
            row=report[kind][0];prefix={'geometry':'SUBD_EDGE_GEOMETRY','cases':'EDGE_SEAM_COMMAND','getters':'EDGE_SEAM_GETTER'}[kind]
            original=prefix+' '+json.dumps(row);changed=prefix+' '+json.dumps(dict(row,**{key:bad}))
            with self.assertRaises(ValueError):summarize('\n'.join(changed if line==original else line for line in lines))


if __name__=='__main__':unittest.main()

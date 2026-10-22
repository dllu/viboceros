"""Immutable conversion/creator discovery and complete kernel qualification."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import unittest
from . import tosubd_cases, creator_types_probe
from .report_tosubd_extrusions import ROOT,HERE,FAMILIES,observations,summarize
from .structural_sdk import validate_operations

class ExtrusionConversionTests(unittest.TestCase):
    def test_fixed_caps_transposition_sdk_and_archived_sources(self):
        flags=dict(tosubd_extrusions='extrusions',tosubd_pipes='pipes',tosubd_conversion_edges='conversion_edges')
        for family,count in FAMILIES+[('creator_types',8)]:
            query=json.loads((HERE/'fixtures'/(family+'.json')).read_text())
            self.assertEqual(query,creator_types_probe.request() if family=='creator_types' else tosubd_cases.request(**{flags[family]:True}))
            validate_operations(query)
            native=json.loads((HERE/'observations'/(family+'.json')).read_text())
            self.assertEqual(len(native['results']),count)
            self.assertEqual([r['id']for r in native['results']],[o['id']for o in query['operations']])
            self.assertEqual(native['provenance']['request_sha256'],hashlib.sha256((HERE/'fixtures'/(family+'.json')).read_bytes()).hexdigest())
            for row in native['results']:
                self.assertEqual(row['value']['input'],creator_types_probe.CASES[row['id']] if family=='creator_types' else tosubd_cases.all_cases()[row['id']])
            for name,sha in native['provenance']['sources'].items():
                data=(HERE/name).read_bytes()
                if hashlib.sha256(data).hexdigest()!=sha:data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
        self.assertEqual(len(observations()[0]),67)

    def lines(self):
        report=json.loads((ROOT/'docs/tosubd-extrusion-qualification.json').read_text())
        return report,['TOSUBD_'+stage.upper()+' '+json.dumps(row)for stage,rows in report['stages'].items()for row in rows]

    def test_complete_report_is_order_independent_and_current(self):
        report,lines=self.lines()
        self.assertEqual(summarize('\n'.join(lines)),report)
        self.assertEqual(summarize('\n'.join(reversed(lines))),report)
        self.assertEqual((report['extrusion_kernel_comparisons'],report['direct_surface_sdk_comparisons']),(54,8))
        for name in ['command_qualified','getter_qualified','full_native_parity','performance_qualified']:self.assertIs(report[name],False)
        for path,sha in report['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),sha,path)

    def test_incomplete_or_invalid_geometry_cannot_qualify(self):
        report,lines=self.lines()
        for data in [lines[:-1],lines+lines[:1]]:
            with self.assertRaises(ValueError):summarize('\n'.join(data))
        for stage,rows in report['stages'].items():
            row=rows[0];original='TOSUBD_'+stage.upper()+' '+json.dumps(row)
            for key,bad in [('qualified',False),('faces',True),('maximum_control_or_limit_point_error',float('nan')),('maximum_sampled_surface_error',1.1e-6),('surface_facets_correspond',False)]:
                changed=copy.deepcopy(row);changed['check'][key]=bad
                with self.assertRaises(ValueError,msg=(stage,key)):
                    summarize('\n'.join('TOSUBD_'+stage.upper()+' '+json.dumps(changed)if s==original else s for s in lines))

    def test_creator_discovery_is_strictly_fixed(self):
        query=creator_types_probe.request()
        for edit in [dict(case='arbitrary'),dict(extra=True),dict(id='wrong'),dict(case=[])]:
            bad=copy.deepcopy(query);bad['operations'][0].update(edit)
            with self.assertRaises(ValueError):validate_operations(bad)

if __name__=='__main__':unittest.main()

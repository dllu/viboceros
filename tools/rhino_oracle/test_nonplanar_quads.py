"""Independent native thresholds, immutable inputs and strict replay evidence."""
from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import gzip
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .nonplanar_quad_cases import CASES, request, validate
from .report_nonplanar_quads import FAMILIES, HERE, ROOT, native, summarize
from .structural_sdk import validate_operations, stage_helpers


class NonplanarQuadTests(unittest.TestCase):
    def test_independent_requests_and_frozen_producers(self):
        self.assertEqual(len(native()[1]),42)
        for family in FAMILIES:
            query=HERE/'fixtures'/(family+'.json');data=json.loads((HERE/'observations'/(family+'.json')).read_text())
            payload=request(ngons=family.endswith('ngons'),angles=family.endswith('angles'));self.assertEqual(json.loads(query.read_text()),payload);validate_operations(payload)
            self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(query.read_bytes()).hexdigest())
            for digest in data['provenance']['sources'].values():
                self.assertEqual(hashlib.sha256(gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())).hexdigest(),digest)

    def test_strict_threshold_neighbors_zero_and_either_active_test(self):
        rows=native()[1]
        self.assertEqual(CASES['nonplanar_distance_below']['distance'],math.nextafter(.25,0.))
        self.assertEqual(CASES['nonplanar_distance_above']['distance'],math.nextafter(.25,1.))
        for key in ['flat_distance_zero','flat_angle_zero','flat_both_zero','distance_equal','distance_above','both_neither']:
            self.assertEqual(rows['nonplanar_'+key]['converted'],0)
        for key in ['distance_below','both_distance_only','both_angle_only','tiny']:
            self.assertEqual(rows['nonplanar_'+key]['converted'],1)
        for name in ['ShortestDiagonal','LongestDiagonal','MinimizeArea','MaximizeArea','MinimumAngle','MaximumAngle']:
            self.assertEqual(rows['nonplanar_angle_'+name]['converted'],1)
            self.assertEqual(rows['nonplanar_angle_discriminator_'+name]['converted'],1)

    def test_removed_slots_and_unaffected_ngon_boundaries_are_explicit(self):
        rows=native()[1]
        for key in ['channels_ngon','multiple_faces']:
            self.assertEqual(rows['nonplanar_'+key]['after']['ngons'],[dict(vertices=[],faces=[])])
        for name,index in [('reverse_owners',0),('preserved_first',0),('preserved_middle',1)]:
            value=rows['nonplanar_ngons_'+name]['after'];survivors=[g for g in value['ngons']if g['faces']]
            self.assertEqual(survivors,[dict(vertices=[8,9,10,11],faces=[2])]);self.assertEqual(value['ngons'][index],survivors[0])
        self.assertTrue(all(not g['faces']for g in rows['nonplanar_ngons_all_removed']['after']['ngons']))

    def test_allowlist_standalone_worker_and_immutable_recaptures(self):
        query=request()
        for op in [None,{},[],dict(query['operations'][0],case='arbitrary'),dict(query['operations'][0],path='arbitrary')]:
            with self.assertRaises(ValueError):validate(op)
        with tempfile.TemporaryDirectory()as d:
            stage_helpers(query,HERE,Path(d))
            subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import nonplanar_quad_probe',d],check=True,capture_output=True)
        for family in FAMILIES:
            paths=[HERE/d/(family+'.json')for d in ['fixtures','observations']];before=[p.read_bytes()for p in paths]
            result=subprocess.run([sys.executable,'-m','tools.rhino_oracle.generate_nonplanar_quads']+(['--angles']if family.endswith('angles')else['--ngons']if family.endswith('ngons')else[]),cwd=ROOT,text=True,capture_output=True)
            self.assertNotEqual(result.returncode,0);self.assertIn('observations are immutable',result.stderr)
            self.assertEqual([p.read_bytes()for p in paths],before)

    def test_failed_import_retains_evidence_without_qualified_geometry(self):
        data=json.loads((HERE/'observations/nonplanar_quad_import_failure.json').read_text())
        self.assertEqual(data['status'],'worker_import_failure');self.assertFalse(data['response_retained'])
        self.assertEqual((data['requested_workflows'],data['qualified_workflows']),(32,0))
        for key,path in [('request_sha256','fixtures/nonplanar_quad_conversion.json'),('log_sha256','observations/nonplanar_quad_import_failure.log')]:
            self.assertEqual(data[key],hashlib.sha256((HERE/path).read_bytes()).hexdigest())
        for digest in data['sources'].values():
            self.assertEqual(hashlib.sha256(gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())).hexdigest(),digest)
        self.assertFalse((HERE/'observations/nonplanar_quad_conversion.json').exists())

    def evidence(self):
        data=json.loads((ROOT/'docs/nonplanar-quad-qualification.json').read_text())
        return data,['NONPLANAR_QUAD '+json.dumps(r)for r in data['cases']]

    def test_report_reconstructs_without_claiming_native_command_parity(self):
        data,lines=self.evidence();self.assertEqual(reconstruct_measurement(data, summarize, '\n'.join(reversed(lines))),data)
        self.assertEqual((data['sdk_geometry_cases'],data['local_command_cases']),(42,42))
        self.assertFalse(data['full_native_parity']);self.assertFalse(data['native_command_workflow_qualified']);self.assertFalse(data['performance_qualified'])

    def test_missing_duplicate_nonfinite_and_relabelled_replays_fail(self):
        data,lines=self.evidence()
        for text in ['\n'.join(lines[1:]),'\n'.join(lines+lines[:1])]:
            with self.assertRaises(ValueError):summarize(text)
        for row in [data['cases'][0],next(r for r in data['cases']if r['id']=='nonplanar_channels_ngon')]:
            needle='NONPLANAR_QUAD '+json.dumps(row);remaining=[s for s in lines if s!=needle]
            edits=[('qualified',False),('raw_faces',False),('channels',False),('metadata',False),('selection',False),('history',False),('vertices',True),('converted',False),('native_invalid_ngons',[0]),('maximum_error',float('nan')),('maximum_error',1e-3)]
            if row['native_empty_ngon_slots']:edits += [('native_empty_ngon_slots',[]),('ngons',1)]
            for key,value in edits:
                bad=deepcopy(row);bad[key]=value
                with self.assertRaises(ValueError):summarize('\n'.join(remaining+['NONPLANAR_QUAD '+json.dumps(bad)]))
if __name__=='__main__':unittest.main()

"""Static mesh rebuilding, staged options and strict evidence boundaries."""
from copy import deepcopy
import base64
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .rebuild_mesh_cases import request,validate
from .report_rebuild_mesh import HERE,ROOT,FAMILIES,EXCLUDED,native,summarize
from .structural_sdk import validate_operations,stage_helpers


class RebuildMeshTests(unittest.TestCase):
    def test_fixed_inputs_frozen_producers_and_exact_source_files(self):
        version,groups=native();self.assertEqual((len(groups['sdk']),len(groups['command'])),(15,32))
        for family,count in FAMILIES.items():
            flags={family.removeprefix('rebuild_mesh_'):True}if family!='rebuild_mesh_sdk'else{}
            if 'command'in flags:flags={'commands':True}
            payload=request(**flags);validate_operations(payload);query=HERE/'fixtures'/(family+'.json');data=json.loads((HERE/'observations'/(family+'.json')).read_text());self.assertEqual(json.loads(query.read_text()),payload);self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(query.read_bytes()).hexdigest())
            for digest in data['provenance']['sources'].values():self.assertEqual(hashlib.sha256(gzip.decompress((HERE/'observations/source_snapshots'/(digest+'.gz')).read_bytes())).hexdigest(),digest)
            for row in data['results']:
                if 'command'in row['value']['input']:self.assertEqual((HERE/'fixtures'/family/(row['id']+'.3dm')).read_bytes(),base64.b64decode(row['value']['source_3dm'],validate=True))

    def test_all_preservation_combinations_compaction_and_source_immutability(self):
        rows=native()[1]['sdk']
        for name,uv,colors in [('default',False,False),('neither',False,False),('uv',True,False),('colors',False,True),('both',True,True)]:
            v=rows['rebuild_mesh_'+name];self.assertEqual(bool(v['after']['uv']),uv);self.assertEqual(bool(v['after']['colors']),colors);self.assertEqual(v['after']['ngons'],[]);self.assertEqual(v['before'],v['source_after'])
        v=rows['rebuild_mesh_unused'];self.assertEqual(v['after']['vertices'],[v['before']['vertices'][i]for i in [1,2,4,5]]);self.assertEqual(v['after']['uv'],[v['before']['uv'][i]for i in [1,2,4,5]]);self.assertEqual(v['after']['faces'],[[0,1,2,3]])
        self.assertEqual(rows['rebuild_mesh_translated']['before']['vertices'],rows['rebuild_mesh_translated']['after']['vertices']);self.assertEqual(len(rows['rebuild_mesh_seam']['after']['vertices']),6)

    def test_native_defaults_acceptance_toggles_and_excluded_late_options(self):
        rows=native()[1]['command']
        for name,uv,colors in [('fresh',False,False),('after_both',True,True),('after_uv',True,False),('after_colors',False,True),('after_neither',False,False),('after_cancel',False,False)]:
            self.assertEqual(rows['rebuild_mesh_command_'+name][1],dict(preserve_texture_coordinates=uv,preserve_vertex_colors=colors))
        self.assertEqual(rows['rebuild_mesh_holdout_after_both'][1],dict(preserve_texture_coordinates=True,preserve_vertex_colors=True));self.assertEqual(rows['rebuild_mesh_holdout_after_cancel'][1],dict(preserve_texture_coordinates=False,preserve_vertex_colors=True))
        self.assertNotIn(EXCLUDED,rows);data=json.loads((HERE/'observations/rebuild_mesh_discovery.json').read_text());v=next(r['value']for r in data['results']if r['id']==EXCLUDED);self.assertIn('Unknown command: _PreserveTextureCoordinates=_Yes',v['history']);self.assertEqual(next(o['mesh']['uv']for o in v['after']if o['source']==0),[])

    def test_allowlist_standalone_staging_and_immutable_recaptures(self):
        payload=request(commands=True)
        for op in [None,{},[],dict(payload['operations'][0],case='arbitrary'),dict(payload['operations'][0],path='arbitrary')]:
            with self.assertRaises(ValueError):validate(op)
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(payload,HERE,Path(directory));subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import rebuild_mesh_probe',directory],check=True,capture_output=True)
        for family,flags in [('rebuild_mesh_sdk',[]),('rebuild_mesh_discovery',['--discovery']),('rebuild_mesh_command',['--commands']),('rebuild_mesh_holdouts',['--holdouts'])]:
            paths=[HERE/d/(family+'.json')for d in ['fixtures','observations']];before=[p.read_bytes()for p in paths]
            result=subprocess.run([sys.executable,'-m','tools.rhino_oracle.generate_rebuild_mesh']+flags,cwd=ROOT,text=True,capture_output=True)
            self.assertNotEqual(result.returncode,0);self.assertIn('observations are immutable',result.stderr);self.assertEqual([p.read_bytes()for p in paths],before)

    def evidence(self):
        data=json.loads((ROOT/'docs/rebuild-mesh-qualification.json').read_text())
        return data,['REBUILD_MESH_'+stage.upper()+' '+json.dumps(row)for stage in ['sdk','command','getter']for row in data[stage]]

    def test_report_reconstructs_and_keeps_local_script_extension_explicit(self):
        data,lines=self.evidence();self.assertEqual(summarize('\n'.join(reversed(lines))),data)
        self.assertEqual((data['sdk_cases'],data['native_command_workflows'],data['registered_command_cases'],data['getter_cases'],data['excluded_discovery_workflows']),(15,32,28,32,1));self.assertFalse(data['full_native_parity']);self.assertFalse(data['performance_qualified'])

    def test_missing_duplicate_and_forged_evidence_fail(self):
        data,lines=self.evidence()
        for text in ['\n'.join(lines[1:]),'\n'.join(lines+lines[:1])]:
            with self.assertRaises(ValueError):summarize(text)
        for stage in ['sdk','command','getter']:
            row=data[stage][0];prefix='REBUILD_MESH_'+stage.upper()+' ';remaining=[s for s in lines if s!=prefix+json.dumps(row)]
            edits=[('qualified',False),('channels',False),('normals',False),('ngons',False),('maximum_error',float('nan'))]
            edits += [('vertices',True),('source_unchanged',False),('idempotent',False),('preserve_uv',0)]if stage=='sdk'else[('accepted',False),('staged',False),('history',False),('preferences',dict(row['preferences'],preserve_texture_coordinates=0))]
            for key,value in edits:
                bad=deepcopy(row);bad[key]=value
                with self.assertRaises(ValueError):summarize('\n'.join(remaining+[prefix+json.dumps(bad)]))
if __name__=='__main__':unittest.main()

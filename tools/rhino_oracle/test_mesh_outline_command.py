"""Immutable outline scenes, getter-time cameras and independently derived results."""
from tools.verify_provenance import reconstruct_measurement
from copy import deepcopy
import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from .mesh_outline_command_cases import request, validate
from .report_mesh_outline_command import native, summarize, ROOT, HERE
from .structural_sdk import stage_helpers, validate_operations


class MeshOutlineCommandTests(unittest.TestCase):
    def test_fixed_native_commands_and_standalone_staging(self):
        _,rows,_=native();self.assertEqual(len(rows),39)
        for flag in ['commands','lifecycle','quads']:
            q=request(**({}if flag=='commands'else{flag:True}));validate_operations(q)
            with tempfile.TemporaryDirectory()as folder:
                stage_helpers(q,HERE,Path(folder))
                subprocess.run([sys.executable,'-I','-c','import sys;sys.path.insert(0,sys.argv[1]);import mesh_outline_command_probe',folder],check=True,capture_output=True)
            family='mesh_outline_'+flag
            paths=[HERE/d/(family+'.json')for d in ['fixtures','observations']]
            before=[p.read_bytes()for p in paths]
            result=subprocess.run([sys.executable,'-m','tools.rhino_oracle.generate_mesh_outline','--'+flag],cwd=ROOT,text=True,capture_output=True)
            self.assertNotEqual(result.returncode,0);self.assertIn('observations are immutable',result.stderr)
            self.assertEqual([p.read_bytes()for p in paths],before)
        for invalid in [None,[],{},dict(request()['operations'][0],case='arbitrary'),dict(request()['operations'][0],path='arbitrary')]:
            with self.assertRaises(ValueError):validate(invalid)

    def test_native_selection_zero_result_history_and_getter_camera(self):
        _,rows,_=native()
        v=rows['outline_lifecycle_edge_on_pre']
        self.assertTrue(v['success']);self.assertFalse(any(o['role']=='output'for o in v['after']))
        self.assertFalse(any(o['role']=='witness'for o in v['undo']))
        v=rows['outline_lifecycle_mixed_point']
        self.assertTrue(next(o['selected']for o in v['after']if o['source']==1))
        self.assertFalse(any(o['selected']for o in v['redo']))
        for name,v in rows.items():
            view=v['views']['after'];d=view['direction'];length=math.sqrt(sum(x*x for x in d));d=[x/length for x in d]
            plane=v['plane'];aligned=not view['perspective']and abs(abs(sum(a*b for a,b in zip(d,plane['z'])))-1.)<1e-12
            for o in v['after']:
                if o['role']!='output':continue
                for p in o['polyline']:
                    if aligned:error=sum((a-b)*c for a,b,c in zip(p,plane['origin'],plane['z']))
                    else:
                        near,far=view['frustum'][4:];depth=2./(1./near+1./far)if view['perspective']else(near+far)*.5
                        error=sum((a-b)*c for a,b,c in zip(p,view['location'],d))-depth
                    self.assertLess(abs(error),1e-7,name)
        self.assertNotEqual(rows['outline_lifecycle_warped_front']['views']['before'],rows['outline_lifecycle_warped_front']['views']['after'])
        v=rows['outline_quads_pre'];self.assertEqual([len(p)for p in v['sdk_checks']['0']['original']],[4])
        self.assertEqual([len(p)for p in v['sdk_checks']['0']['projected']],[5])
        v=rows['outline_quads_post'];self.assertEqual([len(p)for p in v['sdk_checks']['0']['original']],[5])

    def evidence(self):
        record=json.loads((ROOT/'docs/mesh-outline-command-qualification.json').read_text())
        lines=['MESH_OUTLINE_'+stage.upper()+' '+json.dumps(row)for stage in ['command','getter']for row in record[stage]]
        return record,lines

    def test_report_recomputes_boundaries_and_keeps_curved_mesh_gap(self):
        record,lines=self.evidence();self.assertEqual(reconstruct_measurement(record, summarize, '\n'.join(reversed(lines))),record)
        self.assertEqual((record['replayed_commands'],record['replayed_getters']),(36,39))
        self.assertEqual((record['fully_matched_commands'],record['fully_matched_getters']),(33,36))
        self.assertEqual(record['surface_render_mesh_gaps'],['outline_lifecycle_sphere'])
        self.assertEqual(record['projection_gaps'],['outline_lifecycle_warped_perspective','outline_quads_post'])
        gap=next(r for r in record['command']if r['id']=='outline_lifecycle_sphere')
        self.assertGreater(gap['maximum_sampled_boundary_deviation'],1e-5)
        self.assertLess(gap['native_render_control_error'],1e-7)
        for bad in [lines[:-1],lines+lines[:1]]:
            with self.assertRaises(ValueError):summarize('\n'.join(bad))

    def test_report_rejects_relabeling_or_stale_lifecycle_and_controls(self):
        record,lines=self.evidence()
        for change in ['geometry','source','metadata','control','boundary']:
            rows=deepcopy(record['command']);row=next(r for r in rows if r['geometry']and any(o['role']=='output'for o in r['after']))
            if change=='geometry':row['geometry']=False
            elif change=='source':row['source_unchanged']=False
            elif change=='metadata':row['redo'][-1]['selected']=True
            elif change=='control':row['native_render_control'][0][0][0]+=.1
            else:row['after'][-1]['polyline'][1][0]+=.1
            altered=['MESH_OUTLINE_COMMAND '+json.dumps(r)for r in rows]+[s for s in lines if s.startswith('MESH_OUTLINE_GETTER')]
            with self.assertRaises(ValueError):summarize('\n'.join(altered))
        rows=deepcopy(record['command']);next(r for r in rows if not r['geometry'])['geometry']=True
        with self.assertRaises(ValueError):summarize('\n'.join(['MESH_OUTLINE_COMMAND '+json.dumps(r)for r in rows]+[s for s in lines if s.startswith('MESH_OUTLINE_GETTER')]))


if __name__=='__main__':unittest.main()

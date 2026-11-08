"""Reject missing offset evidence, false history, unsafe input routing and loose gates."""
import copy
import gzip
import json
import os
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from . import offset_surface_command as recipes, offset_surface_getter_evidence as evidence
from . import report_offset_surface_getter as report
from .check_offset_surface_getter import ROOT, PROOF
from .client import OracleClient, OracleError
from .offset_surface_point_picker import OffsetSurfacePointPicker
from .structural_sdk import stage_helpers, PROBES
from .fillet_zero_history import audit

class OffsetSurfaceGetterTests(unittest.TestCase):
    def test_complete_inventory_accounts_for_discoveries_and_failed_import(self):
        rows,captures,counts,paths=evidence.collect(PROOF)
        self.assertEqual((len(rows),len(captures)),(56,8));self.assertEqual(counts,evidence.EXPECTED)
        self.assertGreater(len(paths),40)
        self.assertTrue(all(o['native_valid'] for v in rows.values() for o in v['after']))

    def test_native_admission_distinguishes_copies_replacements_and_zero_walls(self):
        rows=evidence.collect(PROOF)[0]
        plain=rows['offset_q_plain'];copy_object=plain['after'][-1]
        self.assertEqual(copy_object['attributes']['layer'],'Plane output');self.assertEqual(copy_object['attributes']['name'],'');self.assertEqual(copy_object['attributes']['groups'],[])
        self.assertEqual([o['selected'] for o in plain['after']],[0,2,0])
        replacement=rows['offset_q_solid_delete']['after'][-1]
        self.assertTrue(replacement['guide']);self.assertEqual(replacement['attributes'],plain['before'][-1]['attributes'])
        self.assertEqual(len(rows['offset_q_both']['after']),4)
        near=rows['offset_b_through_near_second']['after'][-2:];self.assertAlmostEqual(near[0]['surfaces'][0]['control_points'][0]['point'][2],.25);self.assertAlmostEqual(near[1]['surfaces'][0]['control_points'][0]['point'][2],.75)
        zero_default=rows['offset_b_zero_distance_default'];self.assertIn('Offset distance <1.000>',zero_default['history']);self.assertEqual(zero_default['after'][-1]['surfaces'][0]['control_points'][0]['point'][2],1.)
        wall=rows['offset_h_zero_solid'];self.assertEqual(len(wall['after']),8)
        self.assertIn('failed to create a solid',wall['history'])
        self.assertEqual([o['brep']['topology']['vertices'] for o in wall['after'][4:]],[2]*4)
        self.assertTrue(all(not o['guide'] for o in rows['offset_f_zero_solid_delete']['after'][1:]))
        self.assertTrue(rows['offset_c_zero_both_delete']['after'][-1]['guide'])

    def test_history_requires_real_complete_pairs_even_for_equal_open_offsets(self):
        rows=evidence.collect(PROOF)[0];v=rows['offset_h_zero']
        self.assertEqual(audit(v,'OffsetSrf'),dict(recorded=True,geometry_changed=True))
        broken=copy.deepcopy(v);broken['undo_observation']['undo_signals']=[s for s in broken['undo_observation']['undo_signals'] if not s['end_undo']]
        with self.assertRaisesRegex(ValueError,'incomplete UndoRedo'):audit(broken,'OffsetSrf')
        self.assertEqual(audit(rows['offset_c_through_cancel'],'OffsetSrf'),dict(recorded=False,geometry_changed=False))

    def test_fixed_recipes_and_standalone_staging_reject_injected_actions(self):
        query=recipes.request(list(recipes.QUALIFIED))
        for op in query['operations']:recipes.validate(op)
        with self.assertRaises(ValueError):recipes.validate(dict(query['operations'][0],actions=['Delete']))
        with tempfile.TemporaryDirectory() as directory:
            job=Path(directory);stage_helpers(query,Path(__file__).parent,job)
            module,deps=PROBES['offset_surface_command']
            self.assertEqual({p.name for p in job.iterdir()},{name+'.py' for name in (module,*deps)})
        picker=OffsetSurfacePointPicker(dict(operations=[dict(id='cancel',actions=['Cancel'])]))
        with patch('tools.rhino_oracle.chamfer_point_picker.subprocess.run') as run:
            self.assertTrue(picker.send_input('@offset-srf-escape:cancel-0',0,0,'1234'))
            self.assertEqual(run.call_args.args[0],['xdotool','windowactivate','--sync','1234','key','--clearmodifiers','Escape'])

    def test_live_client_rejects_desktop_display_before_launch(self):
        with patch.dict(os.environ,{'DISPLAY':':1','VIBOCEROS_ORACLE_HEADLESS':':99'}),patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaisesRegex(OracleError,'dedicated Xvfb'):OracleClient(launcher='/bin/true').run_rhino(recipes.request(['offset_q_plain']))
        launch.assert_not_called()

    def test_failed_capture_receipt_cannot_qualify_observations(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'captures.json';receipts=json.loads(p.read_bytes());receipts[0]['exit_status']=1;p.write_text(json.dumps(receipts))
            with self.assertRaisesRegex(ValueError,'headless capture receipt'):evidence.collect(proof)

    def test_missing_duplicate_or_nonfinite_comparison_fails(self):
        if not (PROOF/'command-native.log.gz').exists():self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'command-native.log.gz';lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,line in enumerate(lines) if line.startswith('OFFSET_SURFACE_COMMAND '));row=json.loads(lines[index].split(' ',1)[1])
            for value in [float('nan'),-1.,1.1e-6]:
                changed=lines.copy();changed[index]='OFFSET_SURFACE_COMMAND '+json.dumps(dict(row,maximum_geometry_error=value))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'geometry gate'):report.comparisons(proof)
            for changed in [lines[:index]+lines[index+1:],lines+[lines[index]]]:
                p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.comparisons(proof)

    def test_pinned_record_reproduces(self):
        path=ROOT/'docs/offset-surface-getter-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        record=json.loads(path.read_bytes());self.assertEqual(record,report.build(PROOF,record['implementation_revision']))

if __name__=='__main__':unittest.main()

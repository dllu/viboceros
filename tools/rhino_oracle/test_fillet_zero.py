"""Reject missing zero-radius comparisons and false history/validity claims."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from . import fillet_zero_evidence as evidence, report_fillet_zero as report
from .fillet_zero_history import audit
from .check_fillet_zero import ROOT, PROOF


class FilletZeroTests(unittest.TestCase):
    def test_complete_final_and_discovery_inventory_is_retained(self):
        rows,captures,counts,paths=evidence.collect(PROOF)
        self.assertEqual((len(rows),len(captures)),(40,5))
        self.assertEqual((counts['changed'],counts['geometry_changed'],counts['discovery_observations'],counts['history_control_observations']),(30,29,40,3))
        self.assertGreater(len(paths),40)
        self.assertEqual({key for key,v in rows.items()if any(not o['native_valid']for o in v['after'])},evidence.FINAL_INVALID)

    def test_equal_geometry_replacement_is_an_actual_undo_record(self):
        rows=evidence.collect(PROOF)[0]
        value=rows['fillet_zero_final/verified_zero_meeting_parts']
        self.assertEqual(audit(value),dict(recorded=True,geometry_changed=False))
        self.assertNotEqual([s['serial']for s in value['undo_observation']['undo_signals']if s['end_undo']],[s['serial']for s in value['undo_observation']['redo_signals']if s['end_redo']])

    def test_command_success_cannot_replace_a_real_history_event(self):
        rows=evidence.collect(PROOF)[0]
        value=copy.deepcopy(rows['fillet_zero_final/verified_zero_circle_line_arc'])
        self.assertTrue(value['undo_observation']['undo_command_succeeded'])
        self.assertFalse(audit(value)['recorded'])
        value['undo_observation']['undo_succeeded']=True
        with self.assertRaisesRegex(ValueError,'Undo classification'):audit(value)
        value=copy.deepcopy(rows['fillet_zero_final/verified_zero_meeting_parts'])
        value['undo_observation']['undo_signals']=[s for s in value['undo_observation']['undo_signals']if not s['end_undo']]
        with self.assertRaisesRegex(ValueError,'incomplete UndoRedo'):audit(value)

    def test_failed_capture_cannot_contribute_observations(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            p=proof/'final-capture.json';v=json.loads(p.read_bytes());v['exit_status']=1;p.write_text(json.dumps(v))
            with self.assertRaisesRegex(ValueError,'headless capture'):evidence.collect(proof)
            shutil.copyfile(PROOF/'final-capture.json',p)
            p=proof/'failed-final.json';v=json.loads(p.read_bytes());v['native_observations']=1;p.write_text(json.dumps(v))
            with self.assertRaisesRegex(ValueError,'failed history observer receipt'):evidence.collect(proof)

    def test_missing_and_nonfinite_comparisons_fail_the_gate(self):
        if not(PROOF/'zero-command-portable.log.gz').exists():self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'zero-command-portable.log.gz'
            lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,line in enumerate(lines)if line.startswith('FILLET_ZERO_COMMAND '));row=json.loads(lines[i].split(' ',1)[1])
            for value in [float('nan'),1.1e-6]:
                changed=lines.copy();changed[i]='FILLET_ZERO_COMMAND '+json.dumps(dict(row,maximum_geometry_error=value))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'geometry gate'):report.comparisons(proof)
            p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.comparisons(proof)

    def test_invalid_native_outputs_require_explicit_preservation_guards(self):
        if any(not(PROOF/('zero-'+stage+'-'+mode+'.log.gz')).exists()for stage in ['command','ui']for mode in ['portable','native']):self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'zero-ui-portable.log.gz'
            lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,line in enumerate(lines)if line.startswith('FILLET_ZERO_INVALID_UI '))
            p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete invalid UI guards'):report.comparisons(proof)

    def test_pinned_record_reproduces(self):
        p=ROOT/'docs/fillet-zero-validation.json'
        if not p.exists():self.skipTest('record follows implementation commit')
        v=json.loads(p.read_bytes());self.assertEqual(v,report.build(PROOF,v['implementation_revision']))


if __name__=='__main__':unittest.main()

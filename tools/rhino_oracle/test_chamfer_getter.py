"""Audit native Chamfer evidence and reject incomplete/nonfinite qualifications."""
import gzip
import json
import shutil
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path
from . import chamfer_command, chamfer_getter_evidence as evidence, report_chamfer_getter as report
from .chamfer_point_picker import ChamferPointPicker
from .check_chamfer_getter import ROOT,PROOF
class ChamferGetterTests(unittest.TestCase):
    def test_complete_native_captures_include_cancellations_and_oversized_gap(self):
        rows,captures,counts,paths=evidence.collect(PROOF)
        self.assertEqual((len(rows),len(captures)),(31,3));self.assertEqual((counts['successful'],counts['cancelled']),(21,10));self.assertGreater(len(paths),20)
        self.assertTrue(all(evidence.result(rows[k])=='Success' for k in evidence.UNSUPPORTED))
    def test_recipes_reject_arbitrary_injected_cases(self):
        for op in chamfer_command.request()['operations']:chamfer_command.validate(op)
        for op in [dict(op='chamfer_command',id='x',case='x'),dict(op='chamfer_command',id='chamfer_parts',case='chamfer_parts',actions=['Cancel'])]:
            with self.assertRaises(ValueError):chamfer_command.validate(op)
    def test_escape_delivery_uses_the_verified_window_supplied_by_private_picker(self):
        picker=ChamferPointPicker(dict(operations=[dict(id='cancel',actions=['Cancel'])]))
        with patch('tools.rhino_oracle.chamfer_point_picker.subprocess.run') as run:
            self.assertTrue(picker.send_input('@chamfer-escape:cancel-0',0,0,'1234'))
            self.assertEqual(run.call_args.args[0],['xdotool','windowactivate','--sync','1234','key','--clearmodifiers','Escape'])
    def test_comparison_gate_rejects_missing_and_nonfinite_rows(self):
        if not (PROOF/'command-portable.log.gz').exists():self.skipTest('checks follow implementation')
        with tempfile.TemporaryDirectory(dir=ROOT/'docs') as d:
            proof=Path(d)/'proof';shutil.copytree(PROOF,proof);p=proof/'command-portable.log.gz'
            lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True);i=next(i for i,s in enumerate(lines) if s.startswith('CHAMFER_COMMAND '));row=json.loads(lines[i].split(' ',1)[1])
            for error in [float('nan'),1.1e-6]:
                changed=lines.copy();changed[i]='CHAMFER_COMMAND '+json.dumps(dict(row,maximum_geometry_error=error))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'geometry gate'):report.comparisons(proof)
            p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.comparisons(proof)
    def test_pinned_record_reproduces(self):
        p=ROOT/'docs/chamfer-getter-validation.json'
        if not p.exists():self.skipTest('record follows implementation commit')
        v=json.loads(p.read_bytes());self.assertEqual(v,report.build(PROOF,v['implementation_revision']))
if __name__=='__main__':unittest.main()

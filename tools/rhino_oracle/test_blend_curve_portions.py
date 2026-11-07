"""Temporary-source qualification rejects changed parents and incomplete replays."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import blend_curve_portions_evidence as evidence,report_blend_curve_portions as report
from .check_blend_curve_portions import ROOT,PROOF
from .client import OracleClient,OracleProtocolError

class PortionBlendTests(unittest.TestCase):
    def test_native_inputs_parent_ownership_and_history_are_retained(self):
        rows,captures,picks,stations,derivatives,joins,cancelled,paths=evidence.collect(PROOF)
        self.assertEqual((len(rows),len(captures),picks,stations,derivatives,joins,cancelled),(36,3,80,1155,129,12,1))
        self.assertGreater(len(paths),1200)
        self.assertEqual(evidence.bindings(rows['blend_curve_portion_discovery/portion_self_join']),[('guide0',True),('guide0',True)])
        self.assertEqual(evidence.bindings(rows['blend_curve_portion_discovery/portion_mixed_self_join']),[('guide0',False),('guide0',True)])
        for v in rows.values():
            if 'Join=Yes'in v['prescribed_case']['actions']:
                o=evidence.output(v);whole=[n for n,t in evidence.bindings(v)if n and not t]
                self.assertEqual(o['attributes']['name'],whole[0]if whole else '')

    def test_changed_inputs_parent_geometry_ownership_and_derivatives_fail(self):
        capture=evidence.captures
        def changed(mutate):
            rows,c,p=capture(PROOF);mutate(rows);return rows,c,p
        first='blend_curve_portion_pilot/portion_first'
        def parent(rows):
            for phase in ['after','redo']:rows[first][phase][1]['curve']['control_points'][0]['point'][0]-=1
        def ownership(rows):
            for phase in ['after','redo']:evidence.output(dict(rows['blend_curve_portion_pilot/portion_join'],after=rows['blend_curve_portion_pilot/portion_join'][phase]))['attributes']['groups']=[]
        def derivatives(rows):
            for phase in ['before','undo','after','redo']:rows['blend_curve_portion_holdouts/portion_rational_g3'][phase][1]['derivative_samples'].pop()
        mutations=[
            (lambda r:r[first].update(success=True),'native portion result'),
            (lambda r:r[first]['input_trace'][0].update(completed=0),'incomplete portion workflow'),
            (lambda r:evidence.output(r[first])['samples'][0]['point'].__setitem__(0,float('nan')),'invalid native portion geometry'),
            (parent,'temporary portion parent changed'),(ownership,'portion join ownership'),(derivatives,'missing portion derivative'),
        ]
        for mutate,message in mutations:
            with mock.patch.object(evidence,'captures',side_effect=lambda *a:changed(mutate)):
                with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)

    def test_every_capture_requires_a_successful_terminal_receipt(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            p=proof/'captures.json';rows=json.loads(p.read_bytes());rows[-1]['exit_status']=1;p.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'incomplete higher-order capture'):evidence.collect(proof)

    def test_fixed_unique_private_requests_are_checked_before_launch(self):
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
            for names in evidence.FAMILIES.values():
                request=evidence.recipes.request(names)
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(request)
                request=copy.deepcopy(request);request['operations']*=2
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOraclePortionInvalid').run_rhino(request)
            launch.assert_not_called()

    def test_pinned_record_requires_complete_finite_replays(self):
        p=ROOT/'docs/blend-curve-portions-validation.json'
        if not p.exists():self.skipTest('record follows implementation commit')
        value=json.loads(p.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'portion-ui-portable.log.gz';lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,s in enumerate(lines)if s.startswith('BLEND_CURVE_PORTION_UI '));row=json.loads(lines[i].split(' ',1)[1])
            for field,error,message in [('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_native_sample_error',float('nan'),'geometry gate'),('maximum_normalized_derivative_error',1.1e-6,'geometry gate'),('maximum_parameter_domain_error',2.1e-6,'parameter or derivative'),('derivative_stations',99,'classification'),('accepted',not row['accepted'],'classification'),('archives',0,'archive gate')]:
                changed=lines.copy();changed[i]='BLEND_CURVE_PORTION_UI '+json.dumps(dict(row,**{field:error}))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
            p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
if __name__=='__main__':unittest.main()

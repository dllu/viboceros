"""Native temporary-source ownership, frozen recipes and replay qualification."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import arc_blend_subcurve_evidence as evidence,report_arc_blend_subcurves as report
from .check_arc_blend_subcurves import ROOT,PROOF
from .client import OracleClient,OracleProtocolError

class ArcBlendSubcurveTests(unittest.TestCase):
    def test_all_native_inputs_parents_cancellation_and_captures_are_retained(self):
        rows,captures,picks,stations,paths=evidence.collect(PROOF)
        self.assertEqual((len(rows),len(captures),picks,stations),(29,4,68,2079))
        self.assertGreater(len(paths),100)
        self.assertEqual(sum(v['success']for v in rows.values()),28)
        self.assertTrue(rows['arc_blend_subcurve_discovery/subcurve_cancel']['pending_redo'])
        for name,count in [('subcurve_self_join',3),('subcurve_mixed_self_join',2)]:
            self.assertEqual(len(rows['arc_blend_subcurve_holdouts/'+name]['after']),count)

    def test_fixed_unique_private_requests_do_not_launch_on_invalid_input(self):
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
            for names in evidence.FAMILIES.values():
                request=evidence.recipes.request(names)
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(request)
                for iterations in [True,1.0,0,2]:
                    v=copy.deepcopy(request);v['iterations']=iterations
                    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleSubcurveInvalid').run_rhino(v)
                v=copy.deepcopy(request);v['operations']*=2
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleSubcurveInvalid').run_rhino(v)
                for mutation in [dict(case=None),dict(case=[]),dict(case='foreign'),dict(id='foreign'),dict(script='Line')]:
                    with self.assertRaises(ValueError):evidence.recipes.validate(dict(request['operations'][0],**mutation))
            launch.assert_not_called()

    def test_nonfinite_geometry_changed_parents_and_incomplete_inputs_are_rejected(self):
        capture=evidence.captures
        def changed(mutator):
            rows,captures,paths=capture(PROOF);mutator(rows);return rows,captures,paths
        first='arc_blend_subcurve_discovery/subcurve_first'
        mutations=[
            (lambda rows:rows[first]['after'][-1]['curve']['control_points'][0]['point'].__setitem__(0,float('nan')),'invalid native subcurve geometry'),
            (lambda rows:rows[first]['prescribed_case']['actions'][2].__setitem__(0,-4.0001),'subcurve recipe changed'),
            (lambda rows:rows[first]['input_trace'][0].update(completed=0),'incomplete subcurve workflow'),
            (lambda rows:rows[first]['after'][1]['curve']['control_points'][0]['point'].__setitem__(0,-99.),'subcurve history changed'),
        ]
        for mutation,message in mutations:
            with mock.patch.object(evidence,'captures',side_effect=lambda *a:changed(mutation)):
                with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)
        def corrupt_both(rows):
            for phase in ['after','redo']:
                parent=next(o for o in rows[first][phase]if o['attributes']['name']=='guide0')
                parent['curve']['control_points'][0]['point'][0]=-99.
        with mock.patch.object(evidence,'captures',side_effect=lambda *a:changed(corrupt_both)):
            with self.assertRaisesRegex(ValueError,'temporary parent changed'):evidence.collect(PROOF)

    def test_capture_receipts_require_terminal_success(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'captures.json';rows=json.loads(path.read_bytes());rows[0]['exit_status']=1;path.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'incomplete subcurve capture'):evidence.collect(proof)

    def test_record_reconstruction_requires_finite_complete_replays(self):
        path=ROOT/'docs/arc-blend-subcurves-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'subcurves-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,s in enumerate(lines)if s.startswith('ARC_BLEND_SUBCURVE_UI '));row=json.loads(lines[i].split(' ',1)[1])
            for field,error,message in [('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_native_arc_sample_error',float('nan'),'geometry gate'),('native_stations',0,'subcurve replay classification'),('accepted',not row['accepted'],'subcurve replay classification'),('archives',0,'archive gate')]:
                changed=lines.copy();changed[i]='ARC_BLEND_SUBCURVE_UI '+json.dumps(dict(row,**{field:error}))+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
            path.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
if __name__=='__main__':unittest.main()

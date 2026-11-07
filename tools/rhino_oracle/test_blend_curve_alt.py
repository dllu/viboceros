"""Alt qualification separates input receipts, construction and replay gates."""
import copy,gzip,json,shutil,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import blend_curve_alt_evidence as evidence,report_blend_curve_alt as report
from .check_blend_curve_alt import ROOT,PROOF
from .client import OracleClient,OracleProtocolError
class AltBlendTests(unittest.TestCase):
    def test_inventory_and_modifier_policy_are_reproducible(self):
        workflows,replays,captures,counts,paths=evidence.collect(PROOF)
        self.assertEqual((len(workflows),len(replays),len(captures)),(37,45,4))
        self.assertEqual(counts,dict(physical_picks=151,alt_clicks=50,shift_clicks=3,point_tangent_stations=1452,derivative_stations=183,joined_workflows=1,cancelled_workflows=1))
        self.assertGreater(len(paths),1869)
    def test_changed_physical_modifiers_parent_or_history_are_rejected(self):
        original=evidence.captures;key='blend_curve_alt_verification/verified_alt_select_typed'
        def changed(mutate):
            rows,c,p=original(PROOF);mutate(rows[key]);return rows,c,p
        def parent(v):
            for phase in ['after','redo']:v[phase][1]['surfaces'][0]['control_points'][0]['point'][0]+=1
        changes=[(lambda v:v['input_trace'][0]['clicks'][-1].update(alt_key_down=False),'public Alt/Shift'),(lambda v:v['input_trace'][0].update(completed=0),'incomplete Alt'),(lambda v:v.update(success=True),'command result'),(parent,'surface parent')]
        for mutate,message in changes:
            with mock.patch.object(evidence,'captures',side_effect=lambda *a:changed(mutate)):
                with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)
    def test_nonzero_pilot_cannot_be_promoted_by_editing_its_receipt(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'diagnostics.json';rows=json.loads(p.read_bytes());rows[0]['exit_status']=0;p.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'terminal receipt'):evidence.collect(proof)
    def test_nominal_capture_receipts_and_policy_are_required(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'captures.json';original=p.read_bytes();rows=json.loads(original);rows[0]['exit_status']=143;p.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'incomplete higher-order capture'):evidence.collect(proof)
            p.write_bytes(original);p=proof/'construction-policy.json';rows=json.loads(p.read_bytes());rows['rounded_pick_moves_contact_to_corner']=False;p.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'construction policy'):evidence.collect(proof)
    def test_private_settings_and_fixed_single_recipes_are_enforced(self):
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
            for names in evidence.FAMILIES.values():
                query=evidence.recipes.request(names)
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(query)
                query=copy.deepcopy(query);query['operations']*=2
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleAltInvalid').run_rhino(query)
            launch.assert_not_called()
    def test_pinned_record_rejects_missing_and_out_of_gate_replays(self):
        path=ROOT/'docs/blend-curve-alt-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'alt-ui-portable.log.gz';raw=p.read_bytes();lines=gzip.decompress(raw).decode().splitlines(keepends=True);i=next(i for i,line in enumerate(lines)if line.startswith('BLEND_CURVE_ALT_UI '));row=json.loads(lines[i].split(' ',1)[1])
            for field,error,message in [('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_normalized_derivative_error',float('nan'),'geometry gate'),('maximum_parameter_domain_gate_ratio',1.1,'fractional domain'),('nominal_capture',True,'classification'),('native_stations',0,'classification'),('archives',0,'archive gate')]:
                changed=lines.copy();changed[i]='BLEND_CURVE_ALT_UI '+json.dumps(dict(row,**{field:error}))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
            p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
if __name__=='__main__':unittest.main()

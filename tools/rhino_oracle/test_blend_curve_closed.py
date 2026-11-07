"""Closed-source qualification retains strict geometry and rejection gates."""
import copy,gzip,json,shutil,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import blend_curve_closed_evidence as evidence,report_blend_curve_closed as report
from .check_blend_curve_closed import ROOT,PROOF
from .client import OracleClient,OracleProtocolError
class ClosedBlendTests(unittest.TestCase):
 def test_inventory_source_retention_and_input_policy_are_reproducible(self):
  workflows,captures,counts,paths=evidence.collect(PROOF)
  self.assertEqual((len(workflows),len(captures)),(45,6))
  self.assertEqual(counts,dict(physical_picks=95,point_tangent_stations=44*33,derivative_stations=657,joined_workflows=6,cancelled_workflows=1,closed_parent_checks=47,rejected_options=17,rejection_workflows=8))
  self.assertGreater(len(paths),2100)
 def test_closed_parent_or_rejected_option_changes_are_rejected(self):
  original=evidence.captures;key='blend_curve_closed_ownership/closed_ownership_two_join'
  def changed(mutate):
   rows,c,p=original(PROOF);mutate(rows[key]);return rows,c,p
  def parent(v):
   for phase in ['after','redo']:v[phase][1]['curve']['control_points'][0]['point'][0]+=1
  changes=[(parent,'closed source parent'),(lambda v:v.update(history=v['history'].replace('Unknown command: _Join=Yes','Unknown command: _Bogus')),'rejected option'),(lambda v:v['input_trace'][0].update(completed=0),'incomplete closed')]
  for mutate,message in changes:
   with mock.patch.object(evidence,'captures',side_effect=lambda *a:changed(mutate)):
    with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)
 def test_unacknowledged_input_or_changed_terminal_receipt_cannot_pass(self):
  original=evidence.captures
  def changed(*args):
   rows,c,p=original(*args);next(iter(rows.values()))['input_trace'][0]['clicks'][0]['click_acknowledged']=False;return rows,c,p
  with mock.patch.object(evidence,'captures',side_effect=changed):
   with self.assertRaisesRegex(ValueError,'public input receipt'):evidence.collect(PROOF)
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
   proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'captures.json';rows=json.loads(p.read_bytes());rows[0]['exit_status']=143;p.write_text(json.dumps(rows))
   with self.assertRaisesRegex(ValueError,'incomplete higher-order capture'):evidence.collect(proof)
 def test_failed_pilot_has_no_publishable_observations(self):
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
   proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'capture-attempts.json';rows=json.loads(p.read_bytes());rows[0]['native_observations']=7;p.write_text(json.dumps(rows))
   with self.assertRaisesRegex(ValueError,'failed-capture receipt'):evidence.collect(proof)
 def test_private_settings_and_fixed_single_recipes_are_required(self):
  with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
   for names in evidence.FAMILIES.values():
    query=evidence.recipes.request(names)
    with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(query)
    query=copy.deepcopy(query);query['operations']*=2
    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleClosedInvalid').run_rhino(query)
   launch.assert_not_called()
 def test_pinned_record_rejects_missing_or_out_of_gate_comparisons(self):
  path=ROOT/'docs/blend-curve-closed-validation.json'
  if not path.exists():self.skipTest('record follows implementation commit')
  value=json.loads(path.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
   proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'closed-ui-portable.log.gz';lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True);i=next(i for i,line in enumerate(lines)if line.startswith('BLEND_CURVE_CLOSED_UI '));row=json.loads(lines[i].split(' ',1)[1])
   for field,error,message in [('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_normalized_derivative_error',float('nan'),'geometry gate'),('maximum_parameter_domain_gate_ratio',1.1,'fractional domain'),('maximum_unused_outer_knot_difference',1.,'padding classification'),('rejected_options',1,'classification'),('archives',0,'archive gate')]:
    changed=lines.copy();changed[i]='BLEND_CURVE_CLOSED_UI '+json.dumps(dict(row,**{field:error}))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
    with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
   p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
   with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
   p.write_bytes(gzip.compress(''.join(lines).encode(),mtime=0))
   p=proof/'closed-command-portable.log.gz';lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True);i=next(i for i,line in enumerate(lines)if line.startswith('BLEND_CURVE_CLOSED_PADDING_GUARD '));row=json.loads(lines[i].split(' ',1)[1]);row['active_knot_error']=float('nan');lines[i]='BLEND_CURVE_CLOSED_PADDING_GUARD '+json.dumps(row)+'\n';p.write_bytes(gzip.compress(''.join(lines).encode(),mtime=0))
   with self.assertRaisesRegex(ValueError,'invalid periodic padding guard'):report.build(proof,revision)
if __name__=='__main__':unittest.main()

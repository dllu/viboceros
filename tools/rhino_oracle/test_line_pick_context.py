"""Public input calibration, private capture gates and projected-pick evidence."""
import copy,gzip,hashlib,json,shutil,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import line_pick_context as recipes,report_line_pick_context as report
from .client import OracleClient,OracleProtocolError
from .report_line_pick_context import ROOT,PROOF,HERE,read,collect_native,build
class ProjectedPickTests(unittest.TestCase):
 def test_complete_native_inputs_and_frozen_producers(self):
  matched,captures,picks,error,world_errors,evidence=collect_native(PROOF)
  self.assertEqual((len(matched),picks),(14,28));self.assertLess(error,1e-6)
  self.assertGreater(world_errors['pick_circle_perspective'],1e-4)
  for family,names in report.FAMILIES.items():
   data=read(family);fixture=HERE/('fixtures/'+family+'.json')
   self.assertEqual(json.loads(fixture.read_bytes()),recipes.request(names));self.assertEqual(data['provenance']['request_sha256'],hashlib.sha256(fixture.read_bytes()).hexdigest())
   for digest in data['provenance']['sources'].values():self.assertEqual(hashlib.sha256(gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())).hexdigest(),digest)
 def test_public_point_must_correspond_to_the_native_contact(self):
  def changed(family):
   data=read(family)
   if family=='line_pick_context':data['results'][0]['value']['input_trace'][0]['clicks'][0]['pick_context']['point'][0]+=1e-3
   return data
  with mock.patch.object(report,'read',side_effect=changed):
   with self.assertRaisesRegex(ValueError,'public pick does not match'):collect_native(PROOF)
 def test_legacy_native_output_identity_is_required(self):
  def changed(family):
   data=read(family)
   if family=='line_tangent_rays':
    row=next(r for r in data['results']if r['id']=='ray_fixed_elevated');row['value']['after'][-1]['curve']['control_points'][0]['point'][0]+=1e-3
   return data
  with mock.patch.object(report,'read',side_effect=changed):
   with self.assertRaisesRegex(ValueError,'legacy/corrected native input identity changed'):collect_native(PROOF)
 def test_only_private_unique_bounded_recipes_launch(self):
  request=recipes.request()
  for changes in [dict(case=None),dict(case=[]),dict(case='foreign'),dict(id='foreign'),dict(script='Line')]:
   op=copy.deepcopy(request['operations'][0]);op.update(changes)
   with self.assertRaises(ValueError):recipes.validate(op)
  with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
   with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(request)
   for n in [True,1.0,2]:
    q=copy.deepcopy(request);q['iterations']=n
    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOraclePickInvalid').run_rhino(q)
   q=copy.deepcopy(request);q['operations'].append(q['operations'][0])
   with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOraclePickInvalid').run_rhino(q)
   launch.assert_not_called()
 def test_reconstruction_rejects_missing_replays_and_geometry_errors(self):
  record=ROOT/'docs/line-pick-context-validation.json'
  if not record.exists():self.skipTest('record follows implementation commit')
  value=json.loads(record.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,build(PROOF,revision))
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as d:
   proof=Path(d)/'proof';shutil.copytree(PROOF,proof);p=proof/'pick-ui-portable.log.gz';lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True);index=next(i for i,s in enumerate(lines)if s.startswith('LINE_PICK_CONTEXT_UI '))
   p.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
   with self.assertRaisesRegex(ValueError,'incomplete or duplicate projected-pick replay'):build(proof,revision)
   for error in [1.1e-6,float('nan')]:
    row=json.loads(lines[index].split(' ',1)[1]);row['maximum_coefficient_and_sample_error']=error;changed=lines.copy();changed[index]='LINE_PICK_CONTEXT_UI '+json.dumps(row)+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
    with self.assertRaisesRegex(ValueError,'geometry or archive gate failed'):build(proof,revision)
   p.write_bytes(gzip.compress(''.join(lines).encode(),mtime=0))
   q=proof/'pick-command-portable.log.gz';source_lines=gzip.decompress(q.read_bytes()).decode().splitlines(keepends=True);source_index=next(i for i,s in enumerate(source_lines)if s.startswith('LINE_PICK_CONTEXT_SOURCE '))
   q.write_bytes(gzip.compress(''.join(source_lines[:source_index]+source_lines[source_index+1:]).encode(),mtime=0))
   with self.assertRaisesRegex(ValueError,'incomplete or duplicate projected source query'):build(proof,revision)
   row=json.loads(source_lines[source_index].split(' ',1)[1]);row['maximum_projection_point_and_native_contact_error']=float('nan');changed=source_lines.copy();changed[source_index]='LINE_PICK_CONTEXT_SOURCE '+json.dumps(row)+'\n';q.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
   with self.assertRaisesRegex(ValueError,'source projection gate failed'):build(proof,revision)
if __name__=='__main__':unittest.main()

"""Native mouse-mode receipts, rejected searches and reconstruction gates."""
import copy,gzip,hashlib,json,math,shutil,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import line_tangent_rays as ui,tangent_zero_sdk as zero
from .client import OracleClient,OracleProtocolError
from . import report_line_tangent_rays as report
from .report_line_tangent_rays import ROOT,PROOF,HERE,read,collect_native,build
class TangentMouseTests(unittest.TestCase):
 def test_frozen_sources_and_complete_native_mouse_modes(self):
  matched,known,diagnostics,captures,picks,queries,trials,failures,evidence=collect_native(PROOF)
  self.assertEqual((len(matched),len(known),len(diagnostics),picks,queries,trials,failures),(11,1,8,22,2002,12012,30))
  self.assertEqual({k for k,v in matched.items()if v['success']},{'ray_planar_elevated','ray_arc_boundary','ray_line_boundary'})
  for k,v in matched.items():
   t=v['input_trace'][0]
   if k.startswith('mouse_'):
    self.assertFalse(v['history_changed']);self.assertEqual(t['sent'][-1]['token'],'_Cancel');self.assertEqual(v['before'],v['after']);self.assertEqual(len(v['pending_redo']),len(v['before'])+1)
  for capture in captures:
   d=read(capture['family']);self.assertEqual(d['provenance']['request_sha256'],hashlib.sha256((HERE/('fixtures/'+capture['family']+'.json')).read_bytes()).hexdigest())
   for h in d['provenance']['sources'].values():self.assertEqual(hashlib.sha256(gzip.decompress((HERE/('observations/source_snapshots/'+h+'.gz')).read_bytes())).hexdigest(),h)
 def test_public_zero_finder_trials_do_not_reproduce_native_failures(self):
  for row in read('tangent_zero_sdk')['results']:
   values=row['value']['queries'];native_failures=[i for i,q in enumerate(values)if not q['native_success']]
   self.assertEqual(native_failures,list(range(502,520 if row['id']=='zero_arc'else 514)))
   for q in values:
    for t in q['trials']:
     self.assertIsNotNone(t['parameter']);self.assertTrue(math.isfinite(t['parameter']))
     if q['native_success']:self.assertLess(abs(t['parameter']-q['native_parameter']),2e-7)
 def test_trial_bounds_alone_do_not_qualify_a_tangent(self):
  def changed(family):
   value=read(family)
   if family=='tangent_zero_sdk':value['results'][0]['value']['queries'][0]['trials'][0]['parameter']=0.5
   return value
  with mock.patch.object(report,'read',side_effect=changed):
   with self.assertRaisesRegex(ValueError,'zero-search tangent residual changed'):collect_native(PROOF)
 def test_only_private_unique_bounded_recipes_launch(self):
  for request in [ui.request(),ui.confirmation_request(),zero.request()]:
   module=zero if request['operations'][0]['op']=='tangent_zero_sdk'else ui
   for updates in [dict(case=None),dict(case=[]),dict(case='foreign'),dict(id='foreign'),dict(script='Line')]:
    op=copy.deepcopy(request['operations'][0]);op.update(updates)
    with self.assertRaises(ValueError):module.validate(op)
   with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
    with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(request)
    for n in [True,1.0,2]:
     q=copy.deepcopy(request);q['iterations']=n
     with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleMouseInvalid').run_rhino(q)
    q=copy.deepcopy(request);q['operations'].append(q['operations'][0])
    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleMouseInvalid').run_rhino(q)
    launch.assert_not_called()
 def test_reconstruction_rejects_missing_replays_and_geometry_errors(self):
  record=ROOT/'docs/line-tangent-rays-validation.json'
  if not record.exists():self.skipTest('record follows implementation commit')
  value=json.loads(record.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,build(PROOF,revision))
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as d:
   proof=Path(d)/'proof';shutil.copytree(PROOF,proof);p=proof/'tangent-rays-ui-portable.log.gz';lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True);index=next(i for i,s in enumerate(lines)if s.startswith('LINE_TANGENT_RAYS_UI '))
   p.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
   with self.assertRaisesRegex(ValueError,'incomplete or duplicate viewing-ray replay'):build(proof,revision)
   for error in [1.1e-6,float('nan')]:
    row=json.loads(lines[index].split(' ',1)[1]);row['maximum_coefficient_and_sample_error']=error;changed=lines.copy();changed[index]='LINE_TANGENT_RAYS_UI '+json.dumps(row)+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
    with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)
if __name__=='__main__':unittest.main()

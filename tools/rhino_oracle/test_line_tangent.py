"""Tangent physical inputs, native state transitions and proof gates."""
import copy,gzip,hashlib,json,shutil,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import line_tangent_cases as cases
from .client import OracleClient,OracleProtocolError
from .report_line_tangent import HERE,ROOT,PROOF,FAMILIES,DIAGNOSTICS,KNOWN_DIFFERENCES,build,observations
class LineTangentTests(unittest.TestCase):
 def test_prescribed_inputs_and_frozen_producer_closures(self):
  self.assertEqual(json.loads((HERE/'fixtures/line_tangent_sources.json').read_bytes()),cases.CASES)
  for family,count in FAMILIES.items():
   raw=(HERE/('fixtures/'+family+'.json')).read_bytes();data=json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))
   self.assertEqual(len(data['results']),count);self.assertEqual(json.loads(raw),cases.request([r['id']for r in data['results']]))
   self.assertEqual(hashlib.sha256(raw).hexdigest(),data['provenance']['request_sha256'])
   for digest in data['provenance']['sources'].values():self.assertEqual(hashlib.sha256(gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())).hexdigest(),digest)
 def test_complete_native_inputs_and_explicit_geometry_differences(self):
  rows=observations();self.assertEqual(set(rows),set(cases.CASES));self.assertEqual(len(rows),73)
  matched={k:v for k,v in rows.items()if k not in DIAGNOSTICS and k not in KNOWN_DIFFERENCES}
  self.assertEqual(len(matched),55);self.assertEqual(sum(v['success']for v in matched.values()),50)
  clicks=endpoints=0
  for k,v in rows.items():
   self.assertTrue(v['returned_idle']);self.assertEqual(v['after'],v['redo']);self.assertTrue(all(o['native_valid']for o in v['after']))
   if k in DIAGNOSTICS:continue
   t=v['input_trace'][0];actions=cases.CASES[k]['actions'];self.assertFalse(t['errors']);self.assertEqual(t['completed'],len(actions));self.assertEqual(t['completed'],t['expected']);self.assertNotIn('Unknown command',v['history'])
   self.assertEqual(t.get('snap_settings',{}).get('planar',False),cases.CASES[k].get('planar',False))
   self.assertEqual([dict(o,selected=0)for o in v['before']],[dict(o,selected=0)for o in v['undo']])
   for c in t.get('clicks',[]):
    self.assertTrue(c['motion_acknowledged']);self.assertTrue(c['click_acknowledged']);self.assertEqual(c['aim'],actions[c['action']].get('pick',actions[c['action']].get('click')))
    if k in KNOWN_DIFFERENCES:continue
    if 'curve'in c:self.assertIsNotNone(c.get('sight_point',c.get('projected_point')));clicks+=1
    else:endpoints+=1
  self.assertEqual((clicks,endpoints),(57,1))
  control=lambda k,i:rows[k]['after'][-1]['curve']['control_points'][i]['point']
  for k in KNOWN_DIFFERENCES:
   self.assertAlmostEqual(control(k,0)[0],0.);self.assertAlmostEqual(control(k,1)[1],5.)
  self.assertLess(control('circle_pick_top',0)[0],0.);self.assertGreater(control('circle_pick_bottom',0)[0],0.)
  for k in ['planar_circle_complete','planar_circle_elevated','planar_spatial_exact']:self.assertEqual(control(k,0)[2],control(k,1)[2])
  for k in ['oncurve_both_positive','oncurve_both_negative']:self.assertEqual(rows[k]['input_trace'][0]['completed'],6)
 def test_private_unique_bounded_fixed_recipes_are_required_before_launch(self):
  seed=cases.request()
  for changes in [dict(case=None),dict(case='foreign'),dict(id='foreign'),dict(script='Line')]:
   op=copy.deepcopy(seed['operations'][0]);op.update(changes)
   with self.assertRaises(ValueError):cases.validate(op)
  with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
   with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(seed)
   for n in [True,1.0,2]:
    q=copy.deepcopy(seed);q['iterations']=n
    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOraclePerpInvalid').run_rhino(q)
   q=copy.deepcopy(seed);q['operations'].append(q['operations'][0])
   with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOraclePerpInvalid').run_rhino(q)
   launch.assert_not_called()
 def test_reconstruction_rejects_missing_replays_geometry_errors_and_archive_gaps(self):
  path=ROOT/'docs/line-tangent-validation.json'
  if not path.exists():self.skipTest('qualification follows implementation commit')
  record=json.loads(path.read_text());revision=record['implementation_revision'];self.assertEqual(record,build(PROOF,revision))
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
   proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'tangent-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True);index=next(i for i,s in enumerate(lines)if s.startswith('LINE_TANGENT_UI '))
   path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
   with self.assertRaisesRegex(ValueError,'incomplete or duplicate Line Tangent replay'):build(proof,revision)
   def replace(**changes):
    row=json.loads(lines[index].split(' ',1)[1]);row.update(changes);changed=lines.copy();changed[index]='LINE_TANGENT_UI '+json.dumps(row)+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
   for value in [1.1e-6,float('nan')]:
    replace(maximum_coefficient_and_sample_error=value)
    with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)
   replace(archives=0)
   with self.assertRaisesRegex(ValueError,'archive coverage differs'):build(proof,revision)
if __name__=='__main__':unittest.main()

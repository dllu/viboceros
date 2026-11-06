"""Frozen Line Normal sources, accepted native prompts and proof reconstruction."""
import copy,gzip,hashlib,json,shutil,tempfile,unittest
from unittest import mock
from pathlib import Path
from . import line_normal_cases as cases
from .client import OracleClient,OracleProtocolError
from .report_line_normal import HERE,ROOT,PROOF,FAMILIES,DIAGNOSTICS,build,observations
class LineNormalTests(unittest.TestCase):
 def test_prescribed_inputs_and_immutable_producer_closures(self):
  self.assertEqual(json.loads((HERE/'fixtures/line_normal_sources.json').read_bytes()),cases.CASES)
  for family,count in FAMILIES.items():
   raw=(HERE/('fixtures/'+family+'.json')).read_bytes();data=json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))
   self.assertEqual(len(data['results']),count);self.assertEqual(json.loads(raw),cases.request([r['id']for r in data['results']]))
   self.assertEqual(hashlib.sha256(raw).hexdigest(),data['provenance']['request_sha256'])
   for digest in data['provenance']['sources'].values():self.assertEqual(hashlib.sha256(gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())).hexdigest(),digest)
 def test_native_inputs_selection_geometry_and_diagnostic_scope(self):
  rows=observations();self.assertEqual(set(rows),set(cases.CASES));self.assertEqual(len(rows),53);clicks=motions=0
  for k,v in rows.items():
   self.assertTrue(v['returned_idle']);self.assertEqual([dict(o,selected=0)for o in v['created']],[dict(o,selected=0)for o in v['after']]);self.assertEqual(v['redo'],v['after']);self.assertTrue(all(o['native_valid']for o in v['after']));self.assertFalse(any(o['selected']for o in v['after']))
   if k in DIAGNOSTICS:continue
   t=v['input_trace'][0];actions=cases.recipe(k)['actions'];self.assertFalse(t['errors']);self.assertEqual(t['completed'],len(actions));self.assertEqual(t['completed'],t['expected'])
   for c in t.get('clicks',[]):
    self.assertTrue(c['click_acknowledged']);self.assertTrue(c['motion_acknowledged']);self.assertEqual(c['aim'],actions[c['action']]['pick']);self.assertTrue(t['prompts'][c['action']+1].startswith(('Start of line','Middle of line')));clicks+=1
   for m in t['motion']:self.assertTrue(m['acknowledged']);motions+=1
  self.assertEqual((clicks,motions),(44,3))
  endpoint=lambda k:rows[k]['after'][-1]['curve']['control_points'][1]['point']
  self.assertEqual(endpoint('line_point'),[5.,4.,3.]);self.assertEqual(endpoint('polyline_vertex'),[7.,0.,3.])
  self.assertEqual(endpoint('plane_numeric'),endpoint('plane_numeric_hover'));self.assertEqual(endpoint('circle_numeric'),endpoint('circle_numeric_inward'))
  self.assertAlmostEqual(rows['trim_shaded_no']['after'][-1]['curve']['control_points'][0]['point'][0],5.)
  self.assertAlmostEqual(rows['trim_shaded_yes']['after'][-1]['curve']['control_points'][0]['point'][0],8.)
 def test_private_bounded_unique_recipes_are_required_before_launch(self):
  seed=cases.request()
  for changes in [dict(case=None),dict(case='foreign'),dict(id='foreign'),dict(script='Line')]:
   op=copy.deepcopy(seed['operations'][0]);op.update(changes)
   with self.assertRaises(ValueError):cases.validate(op)
  with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
   with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(seed)
   for n in [True,1.0,2]:
    q=copy.deepcopy(seed);q['iterations']=n
    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleLineNormalInvalid').run_rhino(q)
   q=copy.deepcopy(seed);q['operations'].append(q['operations'][0])
   with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleLineNormalInvalid').run_rhino(q)
   launch.assert_not_called()
 def test_reconstruction_rejects_missing_replays_errors_and_archive_gaps(self):
  path=ROOT/'docs/line-normal-validation.json'
  if not path.exists():self.skipTest('qualification follows implementation commit')
  record=json.loads(path.read_text());revision=record['implementation_revision'];self.assertEqual(record,build(PROOF,revision))
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
   proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'normal-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True);index=next(i for i,s in enumerate(lines)if s.startswith('LINE_NORMAL_UI '))
   path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
   with self.assertRaisesRegex(ValueError,'incomplete or duplicate Line Normal replay'):build(proof,revision)
   def replace(**changes):
    row=json.loads(lines[index].split(' ',1)[1]);row.update(changes);changed=lines.copy();changed[index]='LINE_NORMAL_UI '+json.dumps(row)+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
   for value in [1.1e-6,float('nan')]:
    replace(maximum_coefficient_and_sample_error=value)
    with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)
   replace(archives=0)
   with self.assertRaisesRegex(ValueError,'archive coverage differs'):build(proof,revision)
if __name__=='__main__':unittest.main()

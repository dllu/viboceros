"""Fixed Line reference input, private picks, immutable captures and proof gates."""
import copy,gzip,hashlib,json,shutil,tempfile,unittest
from pathlib import Path
from unittest import mock
from . import line_reference_cases as cases
from .client import OracleClient,OracleProtocolError
from .report_line_reference import HERE,ROOT,PROOF,FAMILIES,build,observations
class LineReferenceTests(unittest.TestCase):
 def test_fixed_sources_and_frozen_capture_closures(self):
  self.assertEqual(json.loads((HERE/'fixtures/line_reference_sources.json').read_bytes()),cases.CASES)
  for family,count in FAMILIES.items():
   raw=(HERE/('fixtures/'+family+'.json')).read_bytes();data=json.loads(gzip.decompress((HERE/('observations/'+family+'.json.gz')).read_bytes()))
   self.assertEqual(len(data['results']),count);self.assertEqual(json.loads(raw),cases.request([r['id']for r in data['results']]))
   self.assertEqual(hashlib.sha256(raw).hexdigest(),data['provenance']['request_sha256'])
   for digest in data['provenance']['sources'].values():self.assertEqual(hashlib.sha256(gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())).hexdigest(),digest)
 def test_native_picks_outputs_geometry_history_and_selection(self):
  rows=observations();self.assertEqual(set(rows),set(cases.CASES));self.assertEqual(len(rows),36);self.assertEqual(sum(v['success']for v in rows.values()),30);clicks=0
  for k,v in rows.items():
   self.assertTrue(v['returned_idle']);self.assertEqual([dict(o,selected=0)for o in v['created']],[dict(o,selected=0)for o in v['after']]);self.assertEqual(v['redo'],v['after']);self.assertTrue(all(o['native_valid']for o in v['after']));self.assertFalse(any(o['selected']for o in v['after']))
   t=v['input_trace'][0];actions=cases.recipe(k)['actions'];self.assertFalse(t['errors']);self.assertEqual(t['completed'],len(actions));self.assertEqual(t['completed'],t['expected'])
   self.assertEqual([dict(o,selected=0)for o in v['before']],[dict(o,selected=0)for o in v['undo']])
   for c in t.get('clicks',[]):
    self.assertTrue(c['click_acknowledged']);self.assertTrue(c['motion_acknowledged']);self.assertEqual(c['aim'],actions[c['action']]['pick']);self.assertEqual(c['curve'],actions[c['action']]['curve']);clicks+=1
   for step in t['sent']:
    a=actions[step['action']]
    if isinstance(a,list):self.assertEqual([float(n)for n in step['token'][1:].split(',')],a)
    elif isinstance(a,str):self.assertEqual(step['token'],'_'+a)
    else:self.assertEqual(float(step['token']),a)
  self.assertEqual(clicks,34)
  for k in ['polyline_parameter_first','polycurve_length_domain','linear_nurbs_arclength','cubic_nurbs_arclength']:self.assertEqual(rows[k]['after'][-1]['curve']['control_points'][0]['point'],[101.,1.,0.])
  self.assertEqual(rows['polyline_parameter_last']['after'][-1]['curve']['control_points'][0]['point'],[0.,0.,0.])
  self.assertEqual(rows['enter_length']['after'][-1]['curve']['control_points'][1]['point'],[5.,0.,0.])
  for k in ['redo_cancel','redo_empty']:self.assertEqual(rows[k]['pending_redo'][-1]['point'],[90.,91.,92.])
  self.assertEqual(rows['hairpin_first_half']['after'][-1]['curve']['control_points'][0]['point'],[0.,0.,0.])
  self.assertEqual(rows['closed_polyline_end']['after'][-1]['curve']['control_points'][1]['point'],[-4.,-3.,0.])
 def test_private_unique_bounded_recipes_are_required_before_launch(self):
  seed=cases.request()
  for changes in [dict(case=None),dict(case='foreign'),dict(id='foreign'),dict(script='Line')]:
   op=copy.deepcopy(seed['operations'][0]);op.update(changes)
   with self.assertRaises(ValueError):cases.validate(op)
  with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
   with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(seed)
   for n in [True,1.0,2]:
    q=copy.deepcopy(seed);q['iterations']=n
    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleLineRefInvalid').run_rhino(q)
   q=copy.deepcopy(seed);q['operations'].append(q['operations'][0])
   with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleLineRefInvalid').run_rhino(q)
   launch.assert_not_called()
 def test_reconstruction_rejects_missing_replays_geometry_errors_and_archive_gaps(self):
  path=ROOT/'docs/line-reference-validation.json'
  if not path.exists():self.skipTest('qualification follows implementation commit')
  record=json.loads(path.read_text());revision=record['implementation_revision'];self.assertEqual(record,build(PROOF,revision));self.assertEqual(record['model_epsilon'],1e-6);self.assertFalse(record['full_native_parity'])
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
   proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'reference-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True);index=next(i for i,s in enumerate(lines)if s.startswith('LINE_REFERENCE_UI '))
   path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
   with self.assertRaisesRegex(ValueError,'incomplete or duplicate Line reference replay'):build(proof,revision)
   def replace(**changes):
    row=json.loads(lines[index].split(' ',1)[1]);row.update(changes);changed=lines.copy();changed[index]='LINE_REFERENCE_UI '+json.dumps(row)+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
   for value in [1.1e-6,float('nan')]:
    replace(maximum_coefficient_and_sample_error=value)
    with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)
   replace(archives=0)
   with self.assertRaisesRegex(ValueError,'archive coverage differs'):build(proof,revision)
if __name__=='__main__':unittest.main()

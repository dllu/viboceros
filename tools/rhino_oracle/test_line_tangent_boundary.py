"""Boundary construction, public contact queries and replay reconstruction gates."""
import copy,gzip,hashlib,json,math,shutil,tempfile,unittest
from unittest import mock
from pathlib import Path
from . import line_tangent_arcs as ui,line_tangent_arcs_sdk as sdk
from .client import OracleClient,OracleProtocolError
from .report_line_tangent_boundary import ROOT,PROOF,build,collect_native
from .tangent_boundary_evidence import HERE,FAMILIES,UI_FAMILIES,KNOWN,DIAGNOSTICS,read,observations,sdk_variants
class TangentBoundaryTests(unittest.TestCase):
 def test_frozen_sources_and_request_coverage(self):
  self.assertEqual(json.loads((HERE/'fixtures/line_tangent_arcs_sources.json').read_bytes()),ui.CASES)
  self.assertEqual(json.loads((HERE/'fixtures/line_tangent_arcs_sdk_sources.json').read_bytes()),sdk.CASES)
  ids=[]
  for family,count in FAMILIES.items():
   data=read(family);names=[r['id']for r in data['results']];raw=(HERE/('fixtures/'+family+'.json')).read_bytes();self.assertEqual(len(names),count)
   self.assertEqual(json.loads(raw),(ui.request if family in UI_FAMILIES else sdk.request)(names));self.assertEqual(hashlib.sha256(raw).hexdigest(),data['provenance']['request_sha256'])
   for digest in data['provenance']['sources'].values():self.assertEqual(hashlib.sha256(gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())).hexdigest(),digest)
   ids.extend(names)
  self.assertEqual(len(ids),50);self.assertEqual(len(set(ids)),50)
 def test_native_fallbacks_and_seed_failures_remain_explicit(self):
  collect_native(PROOF)
  rows=observations();queries=0
  for family in FAMILIES:
   for row in read(family)['results']:
    k,v=row['id'],row['value'];queries+=sum(len(c['queries'])for c in sdk_variants(v))
    if family not in UI_FAMILIES:continue
    t=v['input_trace'][0];self.assertTrue(v['success']);self.assertTrue(v['returned_idle']);self.assertFalse(t['errors']);self.assertEqual(t['completed'],len(ui.CASES[k]['actions']));self.assertEqual(v['after'],v['redo'])
    for click in t['clicks']:
     self.assertTrue(click['motion_acknowledged']);self.assertTrue(click['click_acknowledged']);self.assertIsNotNone(click['sight_point'])
    if k in KNOWN:self.assertFalse(v['contact_sdk'][0]['queries'][0]['success'])
  self.assertEqual(queries,34728)
  bad=lambda k,rep=0:{i for i,q in enumerate(rows[k]['contacts'][rep]['queries'][1:])if not q['success']}
  self.assertEqual(bad('dense_arc'),set(range(502,520)));self.assertEqual(bad('dense_nurbs'),set(range(502,514)))
  for k in ['dense_rotated45','dense_yz','dense_scaled']:self.assertEqual(bad(k),bad('dense_arc'))
  self.assertEqual(bad('dense_reversed'),{1000-i for i in bad('dense_arc')})
  self.assertEqual(bad('dense_reparameterized'),bad('dense_nurbs'));self.assertEqual(bad('bounds_refined'),bad('dense_nurbs'));self.assertFalse(bad('bounds_left02'))
  point=lambda k,i:rows[k]['after'][-1]['curve']['control_points'][i]['point']
  self.assertEqual(point('end_line_after',1),[15.,0.,0.]);self.assertEqual(point('boundary_negative_length',1),[5.,0.,0.])
  self.assertEqual(point('boundary_planar_spatial',1)[2],2.);self.assertAlmostEqual(point('boundary_planar_spatial',1)[0],23.726027397260278)
  self.assertEqual(point('boundary_zero_retry',1),[15.,0.,0.])
  self.assertEqual(rows['arc_scaled']['before'][1]['curve']['control_points'][0]['point'][0],2.)
 def test_only_owned_unique_bounded_recipes_launch(self):
  for module in [ui,sdk]:
   seed=module.request()
   for changes in [dict(case=None),dict(case=[]),dict(case='foreign'),dict(id='foreign'),dict(script='Line')]:
    op=copy.deepcopy(seed['operations'][0]);op.update(changes)
    with self.assertRaises(ValueError):module.validate(op)
   with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
    with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(seed)
    for n in [True,1.0,2]:
     q=copy.deepcopy(seed);q['iterations']=n
     with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleBoundaryInvalid').run_rhino(q)
    q=copy.deepcopy(seed);q['operations'].append(q['operations'][0])
    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleBoundaryInvalid').run_rhino(q)
    launch.assert_not_called()
 def test_reconstruction_rejects_missing_replays_and_geometry_errors(self):
  path=ROOT/'docs/line-tangent-boundary-validation.json'
  if not path.exists():self.skipTest('record follows implementation commit')
  record=json.loads(path.read_bytes());revision=record['implementation_revision'];self.assertEqual(record,build(PROOF,revision))
  with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
   proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'tangent-boundary-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True);index=next(i for i,s in enumerate(lines)if s.startswith('LINE_TANGENT_BOUNDARY_UI '))
   path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
   with self.assertRaisesRegex(ValueError,'incomplete or duplicate boundary replay'):build(proof,revision)
   for value in [1.1e-6,float('nan')]:
    row=json.loads(lines[index].split(' ',1)[1]);row['maximum_coefficient_and_sample_error']=value;changed=lines.copy();changed[index]='LINE_TANGENT_BOUNDARY_UI '+json.dumps(row)+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
    with self.assertRaisesRegex(ValueError,'geometry gate failed'):build(proof,revision)
if __name__=='__main__':unittest.main()

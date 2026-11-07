"""Higher surface qualification keeps API, policy and document gates distinct."""
import copy
import gzip
import json
import math
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import blend_curve_surface_high_evidence as evidence,report_blend_curve_surface_high as report
from .blend_curve_surface_policy import characterize
from .check_blend_curve_surface_high import ROOT,PROOF
from .client import OracleClient,OracleProtocolError

class HighSurfaceBlendTests(unittest.TestCase):
    def test_workflows_surface_partials_and_parameter_line_policy_are_retained(self):
        workflows,new,captures,counts,paths=evidence.collect(PROOF)
        self.assertEqual((len(workflows),len(new),len(captures)),(42,40,3))
        self.assertEqual(counts,dict(physical_picks=91,point_tangent_stations=1353,derivative_stations=141,surface_partial_stations=76,joined_workflows=2,cancelled_workflows=1))
        self.assertGreater(len(paths),1690)
        p=characterize(new);self.assertEqual(len(p),36);self.assertLess(max(r['parameter_line_error']for r in p),1e-12);self.assertGreater(max(r['normal_section_error']for r in p),.15)
        fourth=evidence.fourth_policy(new);self.assertAlmostEqual(abs(fourth['source_normalized_fourth_normal']),1.152);self.assertLess(fourth['blend_normalized_fourth_norm'],1e-8)
        lengths=evidence.length_diagnostics(new,PROOF);self.assertEqual(len(lengths),3);self.assertTrue(all(abs(r['native_domain_minus_length'])>1e-8*r['native_domain_span']for r in lengths.values()))

    def test_changed_api_observations_parents_and_history_are_rejected(self):
        capture=evidence.captures;key='blend_curve_surface_high_discovery/surface_high_metric_g3'
        def changed(mutate):
            rows,c,p=capture(PROOF);mutate(rows);return rows,c,p
        def partial(rows):rows[key]['input_trace'][0]['clicks'][0]['edge_pick_context']['candidates'][0]['faces'][0]['third_order']['partials'].pop()
        def parent(rows):
            for phase in ['after','redo']:rows[key][phase][1]['surfaces'][0]['control_points'][0]['point'][0]+=1
        for mutate,message in [(partial,'third surface partials'),(parent,'higher surface parent'),(lambda r:r[key].update(success=True),'native result'),(lambda r:r[key]['input_trace'][0].update(completed=0),'incomplete higher surface')]:
            with mock.patch.object(evidence,'captures',side_effect=lambda *a:changed(mutate)):
                with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)

    def test_only_derived_pick_coordinates_allow_bounded_platform_rounding(self):
        rows,_,_=evidence.captures(PROOF);v=rows['blend_curve_surface_high_holdouts/surface_high_chart_g3'];actual=v['prescribed_case'];expected=evidence.recipes.CASES['surface_high_chart_g3']
        self.assertTrue(evidence.recipe_matches(actual,expected))
        rounded=copy.deepcopy(expected);rounded['actions'][3]['pick'][1]=math.nextafter(rounded['actions'][3]['pick'][1],math.inf);self.assertTrue(evidence.recipe_matches(rounded,expected))
        changed=copy.deepcopy(actual);changed['actions'][3]['pick'][1]+=1e-10;self.assertFalse(evidence.recipe_matches(changed,expected))
        changed=copy.deepcopy(actual);changed['curves'][0]['control_points'][0]['point'][0]+=math.ulp(5.);self.assertFalse(evidence.recipe_matches(changed,expected))
        changed=copy.deepcopy(actual);changed['actions'][0]='Continuity_1=G2';self.assertFalse(evidence.recipe_matches(changed,expected))

    def test_failed_menu_attempt_and_fixed_private_requests_are_required(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'attempts.json';rows=json.loads(p.read_bytes());rows[0]['exit_status']=0;p.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'terminal receipt'):evidence.collect(proof)
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
            for names in evidence.FAMILIES.values():
                query=evidence.recipes.request(names)
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(query)
                query=copy.deepcopy(query);query['operations']*=2
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleSurfaceHighInvalid').run_rhino(query)
            launch.assert_not_called()

    def test_pinned_record_requires_complete_finite_api_and_document_replays(self):
        path=ROOT/'docs/blend-curve-surface-high-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            for log,marker,fields in [('surface-high-ui-portable','BLEND_CURVE_SURFACE_HIGH_UI',[('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_normalized_derivative_error',float('nan'),'geometry gate'),('maximum_parameter_domain_gate_ratio',1.1,'fractional domain'),('domain_qualified',False,'domain classification'),('maximum_independent_length_error',1.1e-7,'long-handle length gate'),('derivative_stations',99,'classification'),('accepted',False,'classification'),('archives',0,'archive gate')]),('surface-third-native','SURFACE_THIRD_NATIVE',[('maximum_partial_relative_error',1.1e-8,'partial replay gate'),('stations',0,'partial replay gate')])]:
                p=proof/(log+'.log.gz');original=p.read_bytes();lines=gzip.decompress(original).decode().splitlines(keepends=True);i=next(i for i,s in enumerate(lines)if s.startswith(marker+' '));row=json.loads(lines[i].split(' ',1)[1])
                for field,error,message in fields:
                    changed=lines.copy();changed[i]=marker+' '+json.dumps(dict(row,**{field:error}))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                    with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
                p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
                p.write_bytes(original)
if __name__=='__main__':unittest.main()

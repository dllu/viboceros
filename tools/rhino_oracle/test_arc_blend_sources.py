"""Native source geometry, scalar semantics, retained failures and replay gates."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import arc_blend_source_evidence as evidence,report_arc_blend_sources as report
from .check_arc_blend_sources import ROOT,PROOF
from .client import OracleClient,OracleProtocolError

class ArcBlendSourceTests(unittest.TestCase):
    def test_native_contacts_quantities_and_point_getters_retain_all_observations(self):
        w,p,d,c,picks,stations,paths=evidence.collect(PROOF)
        self.assertEqual((len(w),len(p),len(d),len(c),picks,stations),(35,2,7,4,98,3498))
        self.assertGreater(len(paths),100)
        for name,expected in [('quantity_coordinate_sum',50.),('quantity_coordinate_division',5.)]:
            curve=next(o['curve']for o in p[name]['after']if'curve'in o)
            self.assertEqual(curve['control_points'][0]['point'],[expected,0.,0.])
        self.assertEqual(evidence.target(w['arc_blend_source_holdouts/quantity_trailing_sum']),50.)
        self.assertEqual(evidence.target(w['arc_blend_source_holdouts/quantity_trailing_division']),5.)

    def test_arc_and_point_requests_remain_private_fixed_unique_and_bounded(self):
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
            for module,names in [(evidence.recipes,names)for names in evidence.FAMILIES.values()]+[(evidence.line_recipes,list(evidence.line_recipes.QUANTITY_INPUT))]:
                request=module.request(names)
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(request)
                for iterations in [True,1.0,0,2]:
                    v=copy.deepcopy(request);v['iterations']=iterations
                    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleSourceInvalid').run_rhino(v)
                v=copy.deepcopy(request);v['operations']*=2
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleSourceInvalid').run_rhino(v)
                for mutation in [dict(case=None),dict(case=[]),dict(case='foreign'),dict(id='foreign'),dict(script='Line')]:
                    with self.assertRaises(ValueError):module.validate(dict(request['operations'][0],**mutation))
            launch.assert_not_called()

    def test_nonfinite_geometry_input_drift_and_diagnostic_promotion_are_rejected(self):
        capture=evidence.captures
        def changed(mutator):
            rows,captures,paths=capture(PROOF);mutator(rows);return rows,captures,paths
        mutations=[
            (lambda rows:rows['arc_blend_sources/source_cubic_default']['after'][-1]['samples'][0]['point'].__setitem__(0,float('nan')),'invalid native source geometry'),
            (lambda rows:rows['arc_blend_sources/source_cubic_default']['prescribed_case']['actions'][1]['pick'].__setitem__(1,3.00356+1e-10),'source recipe changed'),
            (lambda rows:rows['arc_blend_source_holdouts/quantity_nested_unit']['input_trace'][0].update(errors=[]),'nested quantity classification'),
        ]
        for mutation,message in mutations:
            with mock.patch.object(evidence,'captures',side_effect=lambda *a:changed(mutation)):
                with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)

    def test_original_failing_contact_and_failed_producer_receipts_are_required(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'regression.json';data=json.loads(path.read_bytes());path.write_text(json.dumps(dict(data,exit_status=0)))
            with self.assertRaisesRegex(ValueError,'original contact regression'):evidence.collect(proof)
            path.write_text(json.dumps(data));path=proof/'capture-sources-attempt1.log.gz';path.write_bytes(gzip.compress(b'capture succeeded\n',mtime=0))
            with self.assertRaisesRegex(ValueError,'IronPython receipt'):evidence.collect(proof)

    def test_record_reconstruction_requires_finite_complete_and_correctly_parsed_replays(self):
        path=ROOT/'docs/arc-blend-sources-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'sources-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,s in enumerate(lines)if s.startswith('ARC_BLEND_SOURCE_UI '));row=json.loads(lines[i].split(' ',1)[1])
            for field,error,message in [('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_native_arc_sample_error',float('nan'),'geometry gate'),('requested_radius_difference',32.,'requested target changed'),('native_stations',0,'source replay classification'),('warning',not row['warning'],'source replay classification')]:
                changed=lines.copy();changed[i]='ARC_BLEND_SOURCE_UI '+json.dumps(dict(row,**{field:error}))+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
            i=next(i for i,s in enumerate(lines)if s.startswith('ARC_BLEND_SOURCE_POINT_UI '));path.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
if __name__=='__main__':unittest.main()

"""Surface-mode qualification rejects altered inputs and incomplete replays."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import blend_curve_edges_evidence as evidence,report_blend_curve_edges as report
from .check_blend_curve_edges import ROOT,PROOF
from .client import OracleClient,OracleProtocolError

class EdgeBlendTests(unittest.TestCase):
    def test_all_modes_diagnostics_parent_ownership_and_input_receipts_are_retained(self):
        workflows,diagnostics,captures,counts,paths=evidence.collect(PROOF)
        self.assertEqual((len(workflows),len(diagnostics),len(captures)),(41,7,4))
        self.assertEqual(counts,dict(physical_picks=83,boundary_pick_receipts=63,surface_inputs=53,point_tangent_stations=1320,derivative_stations=228,joined_workflows=6,cancelled_workflows=1))
        self.assertGreater(len(paths),1500)
        first=workflows['blend_curve_edge_pilot/edge_plane_end'];surface=workflows['blend_curve_edge_discovery/surface_plane_g2']
        self.assertFalse(evidence.inputs(first)[0][2]);self.assertTrue(evidence.inputs(surface)[0][2])
        self.assertEqual(evidence.output(first)['samples'][0]['tangent'],[0.,1.,0.])
        self.assertEqual(evidence.output(surface)['samples'][0]['tangent'],[1.,0.,0.])
        for name in ['surface_mixed_join','surface_mixed_join_second']:
            self.assertEqual(evidence.output(workflows['blend_curve_edge_holdouts/'+name])['attributes']['name'],'guide1')

    def test_changed_inputs_parent_geometry_and_station_counts_fail(self):
        capture=evidence.captures;key='blend_curve_edge_discovery/surface_plane_g2'
        def changed(mutate):
            rows,c,p=capture(PROOF);mutate(rows);return rows,c,p
        def parent(rows):
            for phase in ['after','redo']:rows[key][phase][1]['surfaces'][0]['control_points'][0]['point'][0]+=1
        def derivative(rows):
            for phase in ['after','redo']:evidence.output(dict(rows[key],after=rows[key][phase]))['derivative_samples'].pop()
        def receipt(rows):rows[key]['input_trace'][0]['clicks'][0]['edge_pick_context']['candidates'][0]['faces'][0]['normal'][2]=0.
        mutations=[(lambda r:r[key].update(success=True),'native edge result'),(lambda r:r[key]['input_trace'][0].update(completed=0),'incomplete edge workflow'),(parent,'supporting surface parent'),(derivative,'missing edge derivatives'),(receipt,'supporting-face input receipt')]
        for mutate,message in mutations:
            with mock.patch.object(evidence,'captures',side_effect=lambda *a:changed(mutate)):
                with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)

    def test_successful_and_failed_capture_receipts_are_required(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'captures.json';rows=json.loads(p.read_bytes());rows[-1]['exit_status']=1;p.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'incomplete higher-order capture'):evidence.collect(proof)
            shutil.copyfile(PROOF/'captures.json',p);p=proof/'attempts.json';rows=json.loads(p.read_bytes());rows[0]['exit_status']=0;p.write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError,'failed edge capture receipt'):evidence.collect(proof)

    def test_requests_are_fixed_unique_and_private_before_launch(self):
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
            for names in evidence.FAMILIES.values():
                query=evidence.recipes.request(names)
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(query)
                query=copy.deepcopy(query);query['operations']*=2
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleEdgesInvalid').run_rhino(query)
            launch.assert_not_called()

    def test_pinned_record_requires_every_finite_geometry_and_history_replay(self):
        path=ROOT/'docs/blend-curve-edges-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);p=proof/'edge-ui-portable.log.gz';lines=gzip.decompress(p.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,s in enumerate(lines)if s.startswith('BLEND_CURVE_EDGES_UI '));row=json.loads(lines[i].split(' ',1)[1])
            for field,error,message in [('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_normalized_derivative_error',float('nan'),'geometry gate'),('maximum_parameter_domain_gate_ratio',1.1,'fractional domain gate'),('maximum_parameter_domain_error',1.,'fractional domain gate'),('derivative_stations',99,'classification'),('accepted',False,'classification'),('archives',0,'archive gate')]:
                changed=lines.copy();changed[i]='BLEND_CURVE_EDGES_UI '+json.dumps(dict(row,**{field:error}))+'\n';p.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
            p.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
if __name__=='__main__':unittest.main()

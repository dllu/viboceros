"""Fixed mesh boundaries, owned capture guards and honest qualification."""
from tools.verify_provenance import reconstruct_measurement, verify_measurement
import copy
import gzip
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from . import mesh_polyline_probe as probe
from .client import OracleClient, OracleProtocolError
from .structural_sdk import validate_operations, stage_helpers
from .report_closed_polyline import ROOT, HERE, STAGES, summarize


class MeshPolylineTests(unittest.TestCase):
    def test_fixed_native_captures_and_source_snapshots(self):
        for suffix,count,factory in [('',21,probe.request),('_variants',10,probe.variant_request)]:
            path=HERE/f'fixtures/mesh_polyline{suffix}.json'
            q=json.loads(path.read_text());self.assertEqual(q,factory());validate_operations(q)
            r=json.loads((HERE/f'observations/mesh_polyline{suffix}.json').read_text())
            self.assertEqual(len(r['results']),count)
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),r['provenance']['request_sha256'])
            for name,sha in r['provenance']['sources'].items():
                data=(HERE/name).read_bytes()
                if hashlib.sha256(data).hexdigest()!=sha:
                    data=gzip.decompress((HERE/'observations/source_snapshots'/(sha+'.gz')).read_bytes())
                self.assertEqual(hashlib.sha256(data).hexdigest(),sha,name)
            for op,row in zip(q['operations'],r['results']):
                self.assertEqual(op['id'],row['id']);self.assertNotIn('error',row)
                v=row['value'];self.assertEqual(v['input'],probe.SCENES[row['id']])
                events=[e for e in v['events']if e['name']=='MeshPolyline']
                self.assertEqual(len(events),1);self.assertIn(events[0]['result'],['Success','Cancel'])
                before={o['source_index']:o for o in v['before']}
                for phase in ['after_script','undo','redo']:
                    for o in v[phase]:
                        if o['role']=='source':
                            target=before[o['source_index']]
                            for key in ['name','layer','color','color_source','crc']:
                                self.assertEqual(o[key],target[key],(row['id'],phase,key))
                        else:
                            self.assertFalse(o['selected']);self.assertEqual(o['layer'],'MeshPolyline source')
                            self.assertEqual(o['color'],[37,83,149]);self.assertEqual(o['color_source'],'ColorFromObject')
                            self.assertIn(o['name'],[b['name']for b in before.values()])

    def test_only_fixed_bounded_private_one_iteration_captures_can_launch(self):
        for patch in [{'id':'arbitrary'},{'case':[]},{'case':'arbitrary'},{'script':'arbitrary'}]:
            q=probe.request();q['operations'][0].update(patch)
            with self.assertRaises(ValueError):validate_operations(q)
        with mock.patch.dict(os.environ,{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as spawn:
            with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(probe.request())
            client=OracleClient(settings_scheme='VibocerosOracleMeshPolylineTest20261021')
            for kind in ['repeat','boolean','count','duplicate']:
                q=probe.request()
                if kind=='repeat':q['iterations']=2
                elif kind=='boolean':q['iterations']=True
                elif kind=='count':q['operations']*=2
                else:q['operations'].append(copy.deepcopy(q['operations'][0]))
                with self.assertRaises(OracleProtocolError):client.run_rhino(q)
            spawn.assert_not_called()

    def test_helper_closure(self):
        with tempfile.TemporaryDirectory()as directory:
            stage_helpers(probe.request(),HERE,Path(directory))
            for name in ['mesh_polyline_probe','join_probe','merge_edges_probe']:
                self.assertEqual((Path(directory)/(name+'.py')).read_bytes(),(HERE/(name+'.py')).read_bytes())

    def report(self):
        r=json.loads((ROOT/'docs/closed-polyline-qualification.json').read_text())
        lines=['CLOSED_POLYLINE_'+stage.upper()+' '+json.dumps(row)
               for stage in STAGES for row in r['stages'][stage]['cases']]
        return r,lines

    def test_report_retains_all_native_gaps_and_historical_sources(self):
        r,lines=self.report();self.assertEqual(reconstruct_measurement(r, summarize, '\n'.join(lines)),r)
        self.assertEqual(r['recipes'],31);self.assertEqual(r['model_epsilon'],1e-6)
        self.assertFalse(r['full_native_parity']);self.assertFalse(r['performance_qualified'])
        self.assertEqual(r['stages']['geometry']['unqualified_recipes'],['self_crossing'])
        self.assertEqual(r['stages']['command']['unqualified_recipes'],['self_crossing','collinear','near_closed','far_closed'])
        for stage in ['preselected','postselected']:
            self.assertEqual(r['stages'][stage]['unqualified_recipes'],['self_crossing'])
        self.assertGreater(verify_measurement(r), 0)

    def test_report_rejects_incomplete_duplicate_nonfinite_and_hidden_gaps(self):
        r,lines=self.report()
        for invalid in [lines[:-1],lines+lines[:1]]:
            with self.assertRaises(ValueError):summarize('\n'.join(invalid))
        for error in [True,-1.,float('nan'),float('inf')]:
            row=copy.deepcopy(r['stages']['geometry']['cases'][0]);row['comparisons'][0]['maximum_bidirectional_sample_error']=error
            invalid=lines[:];invalid[0]='CLOSED_POLYLINE_GEOMETRY '+json.dumps(row)
            with self.assertRaises(ValueError):summarize('\n'.join(invalid))
        index=next(i for i,line in enumerate(lines)if line.startswith('CLOSED_POLYLINE_GEOMETRY ')and json.loads(line.split(' ',1)[1])['id']=='self_crossing')
        row=json.loads(lines[index].split(' ',1)[1]);row['qualified']=True
        invalid=lines[:];invalid[index]='CLOSED_POLYLINE_GEOMETRY '+json.dumps(row)
        with self.assertRaises(ValueError):summarize('\n'.join(invalid))


if __name__=='__main__':unittest.main()

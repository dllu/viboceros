"""Native nested-option inputs, retained research and replay rejection gates."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import arc_blend_getter_evidence as evidence,report_arc_blend_getters as report
from .check_arc_blend_getters import ROOT,PROOF
from .client import OracleClient,OracleProtocolError

class ArcBlendGetterTests(unittest.TestCase):
    def test_native_getters_and_all_partition_investigations_are_retained(self):
        w,r,c,p,e=evidence.collect(PROOF)
        self.assertEqual((len(w),len(r),len(c),p),(9,14,4,28))
        self.assertGreater(len(e),40)
        repeated=evidence.read('arc_blend_roundoff')['results']
        self.assertEqual(repeated[0]['value']['radius_search']['sdk']['fraction'],repeated[2]['value']['radius_search']['sdk']['fraction'])
        v=w['arc_blend_radius_getter_holdouts/radius_active_option_then_enter']
        self.assertIn('RadiusDifference=-5',v['history'])
        arcs=next(o['arc_segments']for o in v['after']if'arc_segments'in o)
        self.assertAlmostEqual(arcs[0]['radius']-arcs[1]['radius'],5.,places=8)

    def test_fixed_private_unique_bounded_requests_for_commands_and_research(self):
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
            for public,families in [(False,evidence.FAMILIES),(True,evidence.PARTITIONS)]:
                module=evidence.sdk if public else evidence.recipes
                for names in families.values():
                    request=module.request(names)
                    with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(request)
                    for iterations in [True,1.0,0,2]:
                        changed=copy.deepcopy(request);changed['iterations']=iterations
                        with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleGetterInvalid').run_rhino(changed)
                    changed=copy.deepcopy(request);changed['operations']*=2
                    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleGetterInvalid').run_rhino(changed)
                    for mutation in [dict(case=None),dict(case=[]),dict(case='foreign'),dict(id='foreign'),dict(script='ArcBlend')]:
                        with self.assertRaises(ValueError):module.validate(dict(request['operations'][0],**mutation))
            launch.assert_not_called()

    def test_nonfinite_independent_samples_cannot_pass_the_capture_audit(self):
        read=evidence.read
        def changed(name):
            value=read(name)
            if name=='arc_blend_radius_getters':
                v=value['results'][0]['value'];next(o for o in v['after']if'arc_segments'in o)['samples'][0]['point'][0]=float('nan');v['redo']=copy.deepcopy(v['after'])
            return value
        with mock.patch.object(evidence,'read',side_effect=changed):
            with self.assertRaisesRegex(ValueError,'invalid native getter geometry'):evidence.collect(PROOF)

    def test_unbounded_research_trials_cannot_pass_as_diagnostics(self):
        read=evidence.read
        def changed(name):
            value=read(name)
            if name=='arc_blend_roundoff':value['results'][0]['value']['radius_search']['sdk']['steps']*=3
            return value
        with mock.patch.object(evidence,'read',side_effect=changed):
            with self.assertRaisesRegex(ValueError,'unbounded search trial'):evidence.collect(PROOF)

    def test_record_reconstruction_requires_every_finite_geometry_replay(self):
        path=ROOT/'docs/arc-blend-getters-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'getter-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,s in enumerate(lines)if s.startswith('ARC_BLEND_GETTER_UI '));row=json.loads(lines[i].split(' ',1)[1])
            for field,error,message in [('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_native_arc_sample_error',float('nan'),'geometry gate'),('warning',not row['warning'],'getter warning')]:
                changed=lines.copy();changed[i]='ARC_BLEND_GETTER_UI '+json.dumps(dict(row,**{field:error}))+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
            path.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)

if __name__=='__main__':unittest.main()

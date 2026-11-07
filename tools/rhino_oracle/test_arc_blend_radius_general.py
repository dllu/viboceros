"""General target evidence, final-getter warnings and strict diagnostic gates."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import arc_blend_general_evidence as evidence, report_arc_blend_radius_general as report
from .check_arc_blend_radius_general import ROOT,PROOF
from .client import OracleClient,OracleProtocolError

class ArcBlendGeneralRadiusTests(unittest.TestCase):
    def test_all_native_search_inputs_samples_receipts_and_frozen_producers(self):
        workflows,public,captures,picks,paths=evidence.collect(PROOF)
        self.assertEqual((len(workflows),len(public),len(captures),picks),(79,48,4,162))
        self.assertEqual(sum(evidence.final_warning(v)for v in workflows.values()),49)
        self.assertEqual(len(evidence.REPRESENTATION),2)
        self.assertGreater(len(paths),100)

    def test_contact_edits_can_resolve_a_previously_failed_target(self):
        v=next(r['value']for r in evidence.read('arc_blend_radius_general_holdouts')['results']if r['id']=='rg_contact_skew')
        self.assertIn('Unable to achieve',v['history'])
        self.assertFalse(evidence.final_warning(v))
        changed=copy.deepcopy(v)
        changed['history']+='\nUnable to achieve requested radius difference.'
        self.assertTrue(evidence.final_warning(changed))

    def test_new_fixed_recipes_stay_private_unique_and_bounded(self):
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
            for family,names in evidence.FAMILIES.items():
                module=evidence.sdk if family=='arc_blend_radius_sdk'else evidence.recipes
                request=module.request(names)
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(request)
                for iterations in [True,1.0,0,2]:
                    changed=copy.deepcopy(request);changed['iterations']=iterations
                    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleGeneralInvalid').run_rhino(changed)
                changed=copy.deepcopy(request);changed['operations']*=2
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOracleGeneralInvalid').run_rhino(changed)
                for mutation in [dict(case=None),dict(case=[]),dict(case='foreign'),dict(id='foreign'),dict(script='ArcBlend')]:
                    with self.assertRaises(ValueError):module.validate(dict(request['operations'][0],**mutation))
            launch.assert_not_called()

    def test_nonfinite_sdk_or_missing_command_samples_fail_the_audit(self):
        read=evidence.read
        for family in ['arc_blend_radius_sdk','arc_blend_radius_general']:
            def changed(name):
                data=read(name)
                if name==family:
                    v=data['results'][0]['value']
                    if name=='arc_blend_radius_sdk':v['samples'][0]['point'][0]=float('nan')
                    else:
                        next(o for o in v['after']if 'arc_segments'in o)['samples'].pop()
                        v['redo']=copy.deepcopy(v['after'])
                return data
            with mock.patch.object(evidence,'read',side_effect=changed):
                with self.assertRaisesRegex(ValueError,'measurement|samples'):evidence.collect(PROOF)

    def test_reconstruction_requires_samples_coefficients_and_explicit_partitions(self):
        path=ROOT/'docs/arc-blend-radius-general-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes());revision=value['implementation_revision']
        self.assertEqual(value,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof)
            path=proof/'general-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,s in enumerate(lines)if s.startswith('ARC_BLEND_RADIUS_GENERAL_UI '))
            row=json.loads(lines[index].split(' ',1)[1])
            for field,changed_value,message in [
                ('maximum_native_arc_sample_error',1.1e-6,'geometry gate'),
                ('maximum_native_arc_sample_error',float('nan'),'geometry gate'),
                ('maximum_coefficient_and_sample_error',1.1e-6,'coefficient gate'),
                ('representation_diagnostic',True,'representation classification'),
                ('warning',not row['warning'],'final getter warning'),
            ]:
                changed=lines.copy();changed[index]='ARC_BLEND_RADIUS_GENERAL_UI '+json.dumps(dict(row,**{field:changed_value}))+'\n'
                path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
            path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
            path.write_bytes(gzip.compress(''.join(lines).encode(),mtime=0))
            path=proof/'radius-search-sdk.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            index=next(i for i,s in enumerate(lines)if s.startswith('ARC_BLEND_RADIUS_SEARCH_SDK '))
            path.write_bytes(gzip.compress(''.join(lines[:index]+lines[index+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)

if __name__=='__main__':unittest.main()

"""Remembered native options, cancellation classification and finite replay gates."""
import copy
import gzip
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock
from . import arc_blend_preference_evidence as evidence,report_arc_blend_preferences as report
from .check_arc_blend_preferences import ROOT,PROOF
from .client import OracleClient,OracleProtocolError

class ArcBlendPreferenceTests(unittest.TestCase):
    def test_ordered_native_transitions_and_timeout_diagnostic_remain_distinct(self):
        w,d,c,p,s,e=evidence.collect(PROOF)
        self.assertEqual((len(w),len(d),len(c),p,s),(25,1,3,61,1386))
        self.assertGreater(len(e),80)
        self.assertEqual(sum(v['success']for v in w.values()),18)
        self.assertIn('RadiusDifference=5',w['arc_blend_preference_cancellation/prefs_numeric_twice']['history'])
        self.assertEqual(d[evidence.DIAGNOSTIC]['input_trace'][0]['errors'],['owned ArcBlend input timeout'])

    def test_requests_are_fixed_private_unique_and_bounded(self):
        with mock.patch.dict('os.environ',{'DISPLAY':':999','VIBOCEROS_ORACLE_HEADLESS':':999'}),mock.patch('subprocess.Popen')as launch:
            for names in evidence.FAMILIES.values():
                request=evidence.recipes.request(names)
                with self.assertRaises(OracleProtocolError):OracleClient().run_rhino(request)
                for iterations in [True,1.0,0,2]:
                    v=copy.deepcopy(request);v['iterations']=iterations
                    with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOraclePreferenceInvalid').run_rhino(v)
                v=copy.deepcopy(request);v['operations']*=2
                with self.assertRaises(OracleProtocolError):OracleClient(settings_scheme='VibocerosOraclePreferenceInvalid').run_rhino(v)
                for mutation in [dict(case=None),dict(case=[]),dict(case='foreign'),dict(id='foreign'),dict(script='ArcBlend')]:
                    with self.assertRaises(ValueError):evidence.recipes.validate(dict(request['operations'][0],**mutation))
            launch.assert_not_called()

    def test_geometry_labels_pending_redo_and_diagnostic_classification_are_gated(self):
        capture=evidence.capture_set
        def changed(mutator):
            rows,captures,paths=capture(PROOF,evidence.FAMILIES);mutator(rows)
            return rows,captures,paths
        mutations=[
            (lambda rows: rows[evidence.DIAGNOSTIC]['input_trace'][0].update(errors=[]),'timeout classification'),
            (lambda rows: rows['arc_blend_preferences/prefs_join_commit']['after'][1]['curve']['control_points'][0]['point'].__setitem__(0,float('nan')),'invalid settings geometry'),
            (lambda rows: rows['arc_blend_preferences/prefs_join_default'].update(history=rows['arc_blend_preferences/prefs_join_default']['history'].replace('Join=Yes','Join=No')),'inherited option labels'),
            (lambda rows: rows['arc_blend_preference_holdouts/prefs_pending_cancel'].update(pending_redo=[]),'cancel lost pending redo'),
        ]
        for mutation,message in mutations:
            with mock.patch.object(evidence,'capture_set',side_effect=lambda *a:changed(mutation)):
                with self.assertRaisesRegex(ValueError,message):evidence.collect(PROOF)

    def test_record_reconstruction_requires_every_finite_correctly_classified_replay(self):
        path=ROOT/'docs/arc-blend-preferences-validation.json'
        if not path.exists():self.skipTest('record follows implementation commit')
        value=json.loads(path.read_bytes());revision=value['implementation_revision'];self.assertEqual(value,report.build(PROOF,revision))
        with tempfile.TemporaryDirectory(dir=ROOT/'docs')as directory:
            proof=Path(directory)/'proof';shutil.copytree(PROOF,proof);path=proof/'preferences-ui-portable.log.gz';lines=gzip.decompress(path.read_bytes()).decode().splitlines(keepends=True)
            i=next(i for i,s in enumerate(lines)if s.startswith('ARC_BLEND_PREFERENCES_UI '));row=json.loads(lines[i].split(' ',1)[1])
            for field,error,message in [('maximum_coefficient_and_sample_error',1.1e-6,'geometry gate'),('maximum_native_arc_sample_error',float('nan'),'geometry gate'),('accepted',not row['accepted'],'settings replay classification'),('native_stations',0,'settings replay classification')]:
                changed=lines.copy();changed[i]='ARC_BLEND_PREFERENCES_UI '+json.dumps(dict(row,**{field:error}))+'\n';path.write_bytes(gzip.compress(''.join(changed).encode(),mtime=0))
                with self.assertRaisesRegex(ValueError,message):report.build(proof,revision)
            path.write_bytes(gzip.compress(''.join(lines[:i]+lines[i+1:]).encode(),mtime=0))
            with self.assertRaisesRegex(ValueError,'incomplete or duplicate'):report.build(proof,revision)
if __name__=='__main__':unittest.main()

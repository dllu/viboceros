"""Fresh public numeric getter captures retain picks, failures and originals."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase,mock
from .subcurve_numeric_followup_probe import request,validate_request,run
from .client import OracleClient,OracleError,OracleProtocolError

ROOT=Path(__file__).resolve().parents[2]
def read(name):return json.loads((ROOT/name).read_text())

class SubcurveNumericFollowupTests(TestCase):
    def test_closed_recipe_validation_and_owned_context(self):
        q=request();validate_request(q)
        self.assertEqual(q,read('tools/rhino_oracle/fixtures/subcurve_numeric_followup.json'))
        for change in [dict(case='_Exit'),dict(case=[]),dict(id='x\n_Exit'),dict(extra=True),dict(op='script')]:
            with self.subTest(change=change),self.assertRaises(ValueError):
                validate_request(dict(q,operations=[dict(q['operations'][0],**change)]))
        for change in [dict(protocol_version=True),dict(iterations=True),dict(iterations=2),dict(operations=q['operations']*2)]:
            with self.subTest(change=change),self.assertRaises(ValueError):validate_request(dict(q,**change))
        rhino=mock.Mock();rhino.Commands.Command.InCommand.return_value=True
        with self.assertRaisesRegex(ValueError,'idle execution'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value=False;rhino.RhinoDoc.ActiveDoc.Objects=[object()]
        with self.assertRaisesRegex(ValueError,'empty owned document'):run(q['operations'][0],dict(Rhino=rhino,System=mock.Mock()))
        with mock.patch.dict(os.environ,dict(DISPLAY=':301',VIBOCEROS_ORACLE_HEADLESS=':301'),clear=True),mock.patch('tools.rhino_oracle.client._run_logged') as launch:
            with self.assertRaises((OracleError,OracleProtocolError,ValueError)):OracleClient().run_rhino(q,1)
            launch.assert_not_called()

    def test_native_numeric_confirmation_orientation_and_source_purity(self):
        q=read('tools/rhino_oracle/observations/subcurve_numeric_followup.json')
        self.assertEqual(len(q['results']),29)
        self.assertEqual(sum(r['value']['success'] for r in q['results']),28)
        total=0;rows={}
        for row in q['results']:
            v=row['value'];rows[row['id'].removeprefix('numeric_followup_')]=v
            self.assertFalse(v['command_active'])
            originals={o['source']:o for o in v['before']}
            for o in v['after']:
                if o['source'] is not None:self.assertEqual(o,dict(originals[o['source']],selected=o['selected']))
                else:total+=1;self.assertEqual(len(o['samples']),33)
        self.assertEqual(total,50)
        self.assertFalse(rows['standalone_reference']['success'])
        for name in ['number_only','number_reference','empty_nested','zero','confirm_closed_full']:
            self.assertEqual(sum(o['source'] is None for o in rows[name]['after']),1,name)
        for name,length in [('number_click',2.),('confirm_coordinates',2.),('confirm_reverse',2.),('negative_forward',2.),('replace_number',1.),('confirm_quadratic',2.),('confirm_nonuniform',1.)]:
            output=[o for o in rows[name]['after'] if o['source'] is None][-1]
            self.assertAlmostEqual(output['length'],length,places=6)
        reverse=[o for o in rows['confirm_reverse']['after'] if o['source'] is None][-1]
        self.assertEqual(reverse['samples'][-1],[3.,4.5,0.])
        self.assertLess([o for o in rows['confirm_clamp']['after'] if o['source'] is None][-1]['length'],1.)

    def test_provenance_preserves_reproducible_numeric_followup(self):
        p=read('docs/subcurve-length-confirmation-provenance.json')
        self.assertTrue(p['private_xvfb']);self.assertFalse(p['full_native_parity'])
        self.assertEqual((p['recipes'],p['successful_commands'],p['output_curves']),(29,28,50))
        self.assertEqual(p['settings_scheme'],'VibocerosOracleSubcurveConfirmCommands20261007')
        for name,digest in p['sha256'].items():self.assertEqual(hashlib.sha256((ROOT/name).read_bytes()).hexdigest(),digest,name)

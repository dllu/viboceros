"""Retained Smooth workflows and bounded private-window Escape delivery."""
import hashlib
import json
import os
from pathlib import Path
from unittest import TestCase, mock

from .client import OracleClient, OracleError, OracleProtocolError
from .smooth_workflow_probe import CASES, request, run, validate_request
from .smooth_workflow_input import SmoothKeyboard

ROOT = Path(__file__).resolve().parents[2]


def read(folder, name):
    return json.loads((ROOT / 'tools/rhino_oracle' / folder / (name+'.json')).read_text())


class SmoothWorkflowTests(TestCase):
    def test_closed_schema_and_owned_idle_document_guards(self):
        q = request()
        self.assertEqual(q, read('fixtures', 'smooth_workflow'))
        for change in (dict(extra=True), dict(id='x\n_Delete'), dict(id=[]),
                       dict(id='λ'), dict(case=[]), dict(case='_Exit')):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, operations=[dict(q['operations'][0], **change)]))
        for change in (dict(protocol_version=True), dict(iterations=2),
                       dict(iterations=True), dict(operations=[]), dict(operations=q['operations']*2)):
            with self.subTest(change=change), self.assertRaises(ValueError):
                validate_request(dict(q, **change))
        rhino = mock.Mock()
        rhino.Commands.Command.InCommand.return_value = 1
        with self.assertRaisesRegex(ValueError, 'idle execution'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))
        rhino.Commands.Command.InCommand.return_value = 0
        rhino.RhinoDoc.ActiveDoc.Objects = [object()]
        with self.assertRaisesRegex(ValueError, 'empty owned document'):
            run(q['operations'][0], dict(Rhino=rhino, System=mock.Mock()))

    def test_settings_and_private_display_are_required_before_launch(self):
        for scheme, display, headless in ((None, ':301', ':301'),
                ('VibocerosOracleSmooth', ':1', None),
                ('VibocerosOracleSmooth', ':301', ':302')):
            env = dict(DISPLAY=display)
            if headless: env['VIBOCEROS_ORACLE_HEADLESS'] = headless
            with self.subTest(scheme=scheme, display=display), \
                    mock.patch.dict(os.environ, env, clear=True), \
                    mock.patch('tools.rhino_oracle.client._run_logged') as launch:
                with self.assertRaises((OracleError, OracleProtocolError, ValueError)):
                    OracleClient(settings_scheme=scheme).run_rhino(request(), 1)
                launch.assert_not_called()

    def test_escape_driver_requires_requested_marker_owned_window_and_sends_once(self):
        import tempfile
        q = dict(protocol_version=1, operations=[dict(op='smooth_workflow', id='key', case='keyboard_escape')])
        driver = SmoothKeyboard(q)
        with tempfile.TemporaryDirectory() as root:
            job = Path(root)
            (job/'worker-progress.log').write_text('SMOOTH_ESCAPE key\n')
            with mock.patch('tools.rhino_oracle.smooth_workflow_input.time.monotonic', return_value=0.), \
                    mock.patch('tools.rhino_oracle.smooth_workflow_input.subprocess.run') as send:
                driver(job, {42})
                send.assert_not_called()
            with mock.patch('tools.rhino_oracle.smooth_workflow_input.time.monotonic', return_value=2.), \
                    mock.patch('tools.rhino_oracle.smooth_workflow_input._rhino_window_for_pids', return_value=None), \
                    mock.patch('tools.rhino_oracle.smooth_workflow_input.subprocess.run') as send:
                driver(job, {42})
                send.assert_not_called()
            with mock.patch('tools.rhino_oracle.smooth_workflow_input.time.monotonic', return_value=2.), \
                    mock.patch('tools.rhino_oracle.smooth_workflow_input._rhino_window_for_pids', return_value='0xowned'), \
                    mock.patch('tools.rhino_oracle.smooth_workflow_input.subprocess.run') as send:
                driver(job, {42}); driver(job, {42})
                send.assert_called_once_with(['xdotool','windowactivate','--sync','0xowned','key','--clearmodifiers','Escape'], check=True, timeout=10)
            (job/'worker-progress.log').write_text('SMOOTH_ESCAPE foreign\n')
            with self.assertRaises(OracleProtocolError): driver(job, {42})

    def test_retained_end_results_metadata_cancel_and_undo_redo(self):
        q,r = read('fixtures','smooth_workflow'),read('observations','smooth_workflow')
        self.assertEqual(len(r['results']),26)
        self.assertEqual(r['engine_version'],'8.32.26160.13001')
        self.assertEqual([o['case'] for o in q['operations']],list(CASES))
        self.assertEqual([o['id'] for o in q['operations']],[o['id'] for o in r['results']])
        rows = {o['case']:row['value'] for o,row in zip(q['operations'],r['results'])}
        cancelled = {'unsupported','cancel_factor','cancel_steps','cancel_coordinates','replace','partial_cancel',
                     'keyboard_escape','keyboard_factor','keyboard_selection'}
        for case,v in rows.items():
            with self.subTest(case=case):
                ends=[e for e in v['events'] if e['name']=='Smooth']
                self.assertEqual([e['result'] for e in ends],['Cancel' if case in cancelled else 'Success'])
                self.assertEqual(ends[0]['objects'],v['after'])
                self.assertTrue(all(o['attribute_text']=='attribute' for o in v['after']))
                if case in cancelled:
                    # Admission clears unsupported preselection; compare geometry and attributes.
                    strip=lambda rows:[{k:v for k,v in o.items() if k!='selected'} for o in rows]
                    self.assertEqual(strip(v['before']),strip(v['after']))
                    if 'followup_cancel' in v:
                        self.assertIn('SmoothFactor=0.2  CoordinateSystem=World  X=Yes',v['followup_cancel']['history'])
                if case.startswith('keyboard_'):
                    self.assertEqual(v['host_input'],dict(key='Escape',private_owned_window=True))
        for case in ['accepted','cancelled','numeric_prompt','toggles','polycurve','line','surface','grips_surface','grips_curve']:
            self.assertIsNone(rows[case]['after'][0]['geometry_text'])
        for case in ['mesh','grips_mesh','trimmed_free','trimmed_fixed','grips_trimmed']:
            self.assertEqual(rows[case]['after'][0]['geometry_text'],'geometry')
        v=rows['undo_redo']
        self.assertEqual(v['undo']['after'],v['before'])
        self.assertEqual(v['redo']['after'],v['after_script'])
        self.assertIn('SmoothFactor=0.35  CoordinateSystem=CPlane',v['followup_cancel']['history'])
        for case in ['trimmed_free','trimmed_fixed','grips_trimmed']:
            before,after=rows[case]['before'][0]['brep'],rows[case]['after'][0]['brep']
            self.assertEqual(before['topology'],after['topology'])
            self.assertEqual([t['definition'] for t in before['faces'][0]['loops'][0]],
                             [t['definition'] for t in after['faces'][0]['loops'][0]])

    def test_workflow_provenance(self):
        record=json.loads((ROOT/'docs/smooth-workflow-provenance.json').read_text())
        self.assertEqual(record['native_recipes'],26)
        self.assertEqual(record['application_replays'],26)
        self.assertEqual(record['real_escape_deliveries'],3)
        self.assertTrue(record['private_xvfb'])
        self.assertFalse(record['full_native_parity'])
        for path,expected in record['sha256'].items():
            with self.subTest(path=path):
                self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),expected)

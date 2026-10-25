from copy import deepcopy
import json
import unittest
from unittest.mock import patch
from unittest import mock
from types import SimpleNamespace as NS
from .surface_apex_command_probe import request, validate, run
from .report_surface_apex_native import ROOT, native, summarize


class SurfaceApexNativeTests(unittest.TestCase):
    def test_command_options_validate_before_host_access(self):
        q = request()
        self.assertEqual(len(q['operations']), 72)
        for op in q['operations']:
            validate(op)
        for field, value in [('solid', 1), ('delete_input', None), ('preselect', 'Yes'),
                             ('face', -1), ('apex', [0., float('nan'), 1.]), ('extra', True)]:
            op = deepcopy(q['operations'][0])
            op[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                run(op, {}, 1, q['tolerance'])

    def test_native_capture_validates_sources_and_history(self):
        captures, paths = native()
        self.assertEqual([len(c['results']) for c in captures], [9, 72])
        self.assertTrue(paths)
        for phase in ['after', 'undo', 'redo', 'events']:
            changed = deepcopy(captures[1])
            v = changed['results'][0]['value']
            if phase == 'after':
                v[phase]['source_present'] = False
            elif phase == 'undo':
                v[phase]['breps'] = v['after']['breps']
            elif phase == 'redo':
                v[phase]['source_unchanged'] = False
            else:
                v[phase] = []
            sdk_q = json.loads((ROOT/'tools/rhino_oracle/fixtures/surface_apex_extrusion.json').read_text())
            with self.subTest(phase=phase), patch('tools.rhino_oracle.report_surface_apex_native.json.loads',
                side_effect=[sdk_q, captures[0], request(), changed]), self.assertRaises(ValueError):
                native()

    def test_failed_cancellation_and_disposal_still_release_owned_geometry(self):
        class Surface:
            pass
        source = Surface()
        brep = mock.Mock()
        brep.Faces.Count = 0
        source.ToBrep = mock.Mock(return_value=brep)
        source.Dispose = mock.Mock()
        brep.Dispose.side_effect = ValueError('dispose failed')
        objects = mock.MagicMock()
        objects.__iter__.side_effect = [iter([]), iter([NS(Id='owned')])]
        doc = NS(Objects=objects, UndoRecordingIsActive=False, ModelAbsoluteTolerance=.001,
                 ModelRelativeTolerance=.01, ModelAngleToleranceRadians=.01)
        app = NS(RunScript=mock.Mock(side_effect=ValueError('cancel failed')))
        host = dict(Rhino=NS(Geometry=NS(Surface=Surface, Brep=type('Brep', (), {})),
                            RhinoDoc=NS(ActiveDoc=doc), RhinoApp=app),
                    System=NS(), _object_source=mock.Mock(return_value=source))
        with self.assertRaisesRegex(ValueError, 'cleanup failed'):
            run(request()['operations'][0], host, 1, request()['tolerance'])
        objects.Delete.assert_called_once_with('owned', True)
        brep.Dispose.assert_called_once()
        source.Dispose.assert_called_once()
        self.assertEqual((doc.ModelAbsoluteTolerance, doc.ModelRelativeTolerance, doc.ModelAngleToleranceRadians),
                         (.001, .01, .01))

    def test_report_rebuild_requires_complete_and_exact_measurements(self):
        record = json.loads((ROOT/'docs/surface-apex-native-qualification.json').read_text())
        log = lambda rows, prefix: '\n'.join(prefix+' '+json.dumps(r) for r in rows)
        oracle = log(record['sdk'], 'SURFACE_APEX_SDK')+'\n'+log(record['command'], 'SURFACE_APEX_COMMAND')
        app = log(record['app'], 'SURFACE_APEX_APP')
        self.assertEqual(summarize(oracle, app), record)
        with self.assertRaises(ValueError):
            summarize(oracle, log(record['app'][:-1], 'SURFACE_APEX_APP'))
        with self.assertRaises(ValueError):
            summarize(oracle, app+'\n'+app)
        for field in ['volume', 'face_count', 'is_solid', 'source_present', 'maximum_error', 'bounds', 'breps']:
            rows = deepcopy(record['app'])
            r = rows[0]['after']
            if field in ['volume', 'face_count']:
                r['value']['breps'][0][field] = 1.01
            elif field == 'is_solid':
                r['value']['breps'][0][field] = 1
            elif field == 'source_present':
                r['value'][field] = not r['value'][field]
            elif field == 'bounds':
                r['value']['breps'][0][field][0][0] += .01
            elif field == 'breps':
                r['value'][field] = []
            else:
                r[field] = .01
            with self.subTest(field=field), self.assertRaises(ValueError):
                summarize(oracle, log(rows, 'SURFACE_APEX_APP'))


if __name__ == '__main__':
    unittest.main()

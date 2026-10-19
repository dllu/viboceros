"""Actual command evidence is separate from the parameter-preserving API."""
import copy
import tempfile
from pathlib import Path
from unittest import TestCase
from . import test_remove_control_point as provenance
from .generate_insert_control_point import request
from .generate_insert_control_point_extensions import request as extensions
from .insert_control_point_probe import validate
from .generate_insert_control_point_endpoints import request as endpoints
from .generate_insert_control_point_charts import request as charts
from .structural_sdk import stage_helpers

HERE = Path(__file__).parent


class InsertControlPointCommands(TestCase):
    def capture(self, name, q):
        # Reuse the existing exact recipe, fixture and producer hash checks.
        return provenance.RemoveControlPointCommands.check_capture(self, name, q)

    def test_command_and_extension_captures_have_complete_provenance(self):
        for name, q, count in (('command', request(), 82), ('extensions', extensions(), 24), ('endpoints', endpoints(), 15), ('charts', charts(), 3)):
            self.assertEqual(len(q['operations']), count)
            capture = self.capture('insert_control_point_' + name, q)
            for row in capture['results']:
                v = row['value']; events = [e for e in v['events'] if e['name'] == 'InsertControlPoint']
                self.assertEqual(len(events), 1, row['id'])
                self.assertEqual(v['success'], events[0]['result'] == 'Success')
                self.assertNotIn('Unknown command:', v['history'], row['id'])
                self.assertTrue(v['macro'].startswith('_InsertControlPoint '))
                for key in ('name', 'layer', 'groups', 'user_text'):
                    self.assertEqual(v['before'][0][key], v['after'][0][key], (row['id'], key))
                if not v['success']:
                    self.assertEqual(v['before'][0]['geometry'], v['after'][0]['geometry'], row['id'])

    def test_composite_container_metadata_is_retained(self):
        capture = self.capture('insert_control_point_command', request())
        for row in capture['results']:
            v = row['value']
            if v['recipe']['family'] == 'polycurve':
                self.assertEqual(v['after'][0]['geometry']['type'], 'polycurve')
                self.assertEqual(v['before'][0]['geometry_text'], v['after'][0]['geometry_text'])

    def test_extensions_rebuild_nonrational_nets_and_retain_uv_domains(self):
        capture = self.capture('insert_control_point_extensions', extensions())
        for row in capture['results']:
            v = row['value']
            if not v['success']:
                continue
            old = v['before'][0]['geometry']['surfaces'][0]
            new = v['after'][0]['geometry']['surfaces'][0]
            self.assertTrue(all(p['weight'] == 1. for p in new['control_points']), row['id'])
            self.assertEqual(new['domain_u'], old['domain_u'])
            self.assertEqual(new['domain_v'], old['domain_v'])

    def test_staged_helpers_are_exact_and_bounded(self):
        q = request()
        with tempfile.TemporaryDirectory() as d:
            stage_helpers(q, HERE, Path(d))
            self.assertEqual(len(list(Path(d).iterdir())), 5)
            for p in Path(d).iterdir():
                self.assertEqual(p.read_bytes(), (HERE / p.name).read_bytes())
        for actions in (None, [True], ['Delete'], [dict(world=[1., 2.])],
                        [dict(world=[True, 0., 0.])], [dict(world=[float('nan'), 0., 0.])],
                        [dict(point=[.1, .2, .3])], [dict(point=-1.)], ['Extend'] * 17):
            with self.subTest(actions=actions), self.assertRaises(ValueError):
                validate(dict(copy.deepcopy(q['operations'][0]), actions=actions))

    def test_native_both_rejection_is_separate_diagnostic_evidence(self):
        from .generate_insert_control_point_options import request as options
        capture = self.capture('insert_control_point_options', options())
        self.assertEqual(len(capture['results']), 1)
        value = capture['results'][0]['value']
        self.assertIn('Unknown command: _Direction=_Both', value['history'])
        self.assertTrue(value['success'])  # A subsequent explicit U pick succeeds.

"""Raw native preview images and bounded owned input regression checks."""
import copy
import hashlib
import json
import struct
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

try:
    from PIL import Image
except ImportError:
    Image = None
from .client import OracleProtocolError
from .mirror_preview_probe import validate_request
from .mirror_preview_input import MirrorPreviewPicker

ROOT = Path(__file__).parent


def case():
    return dict(op='mirror_preview', id='case', copy=True, plane='TwoPoint', display_mode='Shaded', finish='Click')


class MirrorPreviewTests(unittest.TestCase):
    def test_prescribed_cases_keep_pending_document_intact_and_commit_expected_bounds(self):
        fixture = json.loads((ROOT / 'fixtures/mirror_preview.json').read_text())
        validate_request(fixture)
        observation = json.loads((ROOT / 'observations/mirror_preview.json').read_text())
        self.assertEqual(len(fixture['operations']), 15)
        self.assertEqual([op['id'] for op in fixture['operations']], [row['id'] for row in observation['results']])
        for op, row in zip(fixture['operations'], observation['results']):
            value = row['value']
            self.assertEqual(value['before'], value['pending'], op['id'])
            self.assertEqual(len(value['before']), 3)
            self.assertTrue(all(item['selected'] for item in value['before']))
            if op['finish'] == 'Cancel':
                self.assertFalse(value['success'])
                self.assertEqual(value['before'], value['after'])
            else:
                self.assertTrue(value['success'])
                reflected = []
                for item in value['before']:
                    lo, hi = item['bounds']
                    reflected.append([[-hi[0], lo[1], lo[2]], [-lo[0], hi[1], hi[2]]])
                expected = reflected + ([item['bounds'] for item in value['before']] if op['copy'] else [])
                self.assertEqual(sorted(expected), sorted(item['bounds'] for item in value['after']))
            image = value['framebuffer']
            path = ROOT / 'observations' / image['path']
            data = path.read_bytes()
            self.assertEqual(hashlib.sha256(data).hexdigest(), image['sha256'])
            self.assertEqual(list(struct.unpack('>II', data[16:24])), image['size'])

    @unittest.skipIf(Image is None, 'requires Pillow for native framebuffer pixel checks')
    def test_native_framebuffers_show_selected_preview_and_copy_dependent_source_style(self):
        fixture = json.loads((ROOT / 'fixtures/mirror_preview.json').read_text())
        observation = json.loads((ROOT / 'observations/mirror_preview.json').read_text())
        for op, row in zip(fixture['operations'], observation['results']):
            value = row['value']
            image = value['framebuffer']
            path = ROOT / 'observations' / image['path']
            with Image.open(path) as bitmap:
                self.assertEqual(list(bitmap.size), image['size'])
                x, y = value['calibration']['regions']['reflected_point']
                pixels = list(bitmap.crop((round(x)-5, round(y)-5, round(x)+6, round(y)+6)).getdata())
                # Selection yellow still exists after moving onto a degenerate
                # point: the native command retains the previous preview.
                self.assertTrue(any(r > 230 and g > 230 and b < 80 for r, g, b in pixels), op['id'])
                x, y = value['calibration']['regions']['source_point']
                pixels = list(bitmap.crop((round(x)-5, round(y)-5, round(x)+6, round(y)+6)).getdata())
                red = any(r > g+50 and r > b+50 for r, g, b in pixels)
                self.assertEqual(red, op['copy'] and op['plane'] == 'TwoPoint', op['id'])

    def test_probe_rejects_unbounded_or_mixed_requests(self):
        for op in [dict(case(), id='case;_Delete'), dict(case(), copy=1), dict(case(), plane='Object'),
                   dict(case(), script='_Delete'), dict(case(), cursor='Degenerate'), dict(case(), display_mode='Other')]:
            with self.assertRaises(ValueError):
                validate_request(dict(protocol_version=1, operations=[op]))
        with self.assertRaises(ValueError):
            validate_request(dict(protocol_version=1, operations=[case(), case()]))
        with self.assertRaises(ValueError):
            validate_request(dict(protocol_version=True, operations=[case()]))

    def test_capture_requires_owned_pid_and_one_completed_image_per_native_result(self):
        picker = MirrorPreviewPicker(dict(protocol_version=1, operations=[case()]))
        with tempfile.TemporaryDirectory() as folder, patch('tools.rhino_oracle.group_picking.subprocess.run') as send:
            job = Path(folder)
            (job / 'worker-progress.log').write_text('PICK @mirror-preview:case 100 100\n')
            picker.ready['@mirror-preview:case'] = 0
            picker(job, set())
            send.assert_not_called()
        response = dict(results=[dict(id='case', value={'native': 'preserved'})])
        with self.assertRaises(OracleProtocolError):
            picker.record_diagnostics(response)
        self.assertEqual(response['results'][0]['value'], {'native': 'preserved'})
        picker.seen.add('@mirror-preview:case')
        picker.images['@mirror-preview:case'] = dict(sha256='abc', size=[2, 2], png_base64='AA==')
        picker.record_diagnostics(response)
        self.assertEqual(response['results'][0]['value']['native'], 'preserved')
        unchanged = copy.deepcopy(response)
        with self.assertRaises(OracleProtocolError):
            picker.record_diagnostics(response)
        self.assertEqual(response, unchanged)

    def test_final_click_waits_for_native_snapshot_and_rejects_changed_window(self):
        picker = MirrorPreviewPicker(dict(protocol_version=1, operations=[case()]))
        name = '@mirror-preview:case'
        picker.images[name] = {'captured': True}
        picker.moved[name] = ('owned', '100', '120', 0)
        with tempfile.TemporaryDirectory() as folder, patch('tools.rhino_oracle.mirror_preview_input.subprocess.run') as send:
            picker.job = Path(folder)
            self.assertFalse(picker.send_input(name, '100', '120', 'owned'))
            send.assert_not_called()
            ready = picker.job / 'mirror-preview-ready-case.json'
            ready.write_text(json.dumps('foreign'))
            with self.assertRaises(OracleProtocolError):
                picker.send_input(name, '100', '120', 'owned')
            send.assert_not_called()
            ready.write_text(json.dumps('case'))
            with self.assertRaises(OracleProtocolError):
                picker.send_input(name, '100', '120', 'foreign')
            send.assert_not_called()
            self.assertTrue(picker.send_input(name, '100', '120', 'owned'))
            self.assertEqual(send.call_args.args[0][-2:], ['click', '1'])


if __name__ == '__main__':
    unittest.main()

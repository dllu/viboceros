"""Prescribed native cursor previews, immutable pending states, and owned input."""
import copy
import hashlib
import json
import math
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

try:
    from PIL import Image
except ImportError:
    Image = None

from .client import OracleProtocolError
from .move_normal_probe import validate_camera
from .translation_input import validate_frame
from .translation_preview_input import TranslationPreviewPicker
from .translation_preview_probe import request, validate_request

ROOT = Path(__file__).parent
REPEATED = ('Repeat', 'FromLastPoint', 'UseLastDistance', 'UseLastDirection')


def destination(op, frame):
    """Independent World XY ray intersection and fixture constraint equations."""
    a, b = frame['ray']
    if op['placement'] == 'Vertical':
        return [0., 0., a[2]]
    t = -a[2] / (b[2] - a[2])
    point = [a[i] + t * (b[i] - a[i]) for i in range(3)]
    point[2] = 0.
    if op['placement'] in ('Normal', 'UseLastDirection'):
        return [point[0], 0., 0.]
    if op['placement'] == 'UseLastDistance':
        length = math.sqrt(sum(v*v for v in point))
        return [6*v/length for v in point]
    return point


def translated(item, delta, selected):
    result = copy.deepcopy(item)
    result['bounds'] = [[value+offset for value, offset in zip(p, delta)] for p in item['bounds']]
    result['selected'] = selected
    if item['vertices'] is not None:
        result['vertices']=[[value+offset for value,offset in zip(p,delta)] for p in item['vertices']]
    return result


def normalized(items):
    return sorted((item['type'], item['reference'], item['selected'], item['bounds'],
                   sorted(item['vertices']) if item['vertices'] is not None else None) for item in items)


class TranslationPreviewTests(unittest.TestCase):
    def captures(self):
        fixture = json.loads((ROOT/'fixtures/translation_preview.json').read_text())
        observation = json.loads((ROOT/'observations/translation_preview.json').read_text())
        self.assertEqual(fixture, request())
        validate_request(fixture)
        self.assertEqual(len(fixture['operations']), 36)
        self.assertEqual([op['id'] for op in fixture['operations']], [row['id'] for row in observation['results']])
        return zip(fixture['operations'], observation['results'])

    def assert_objects_close(self, actual, expected, label):
        actual, expected = normalized(actual), normalized(expected)
        self.assertEqual(len(actual), len(expected), label)
        for a, e in zip(actual, expected):
            self.assertEqual(a[:3], e[:3], label)
            for p, q in zip(a[3], e[3]):
                for x, y in zip(p, q):
                    # Rhino's shaded box bounds include small render-mesh rounding.
                    self.assertAlmostEqual(x, y, delta=5e-7 if a[0]=='Brep' else 1e-9, msg=label)
            if a[4] is not None:
                for p,q in zip(a[4],e[4]):
                    for x,y in zip(p,q):self.assertAlmostEqual(x,y,delta=1e-9,msg=label)

    def test_native_pending_and_completed_geometry_match_prescribed_ray_constraints(self):
        for op, row in self.captures():
            value = row['value']; pending = value['pending']
            validate_frame(pending['frame']); validate_camera(pending['camera'], pending['frame'])
            before = value['before']
            self.assertEqual(len(before), 4 if op['placement'] == 'Normal' else 3)
            expected = copy.deepcopy(before)
            if op['placement']=='Normal':
                # Rhino's GetObject highlights the reference until command completion.
                for item in expected:
                    if item['reference']:item['selected']=True
            if op['placement'] in REPEATED:
                expected += [translated(item, [-6., 0., 0.], False) for item in before]
            self.assert_objects_close(pending['objects'], expected, op['id'])
            if op['finish'] == 'Cancel':
                self.assertFalse(value['success'])
            else:
                self.assertTrue(value['success'])
                base = [3., 0., 0.] if op['placement'] == 'Normal' else [0., 0., 0.]
                delta = [v-b for v, b in zip(destination(op, pending['frame']), base)]
                shifted = [translated(item, delta, op['command'] == 'Move') for item in before if not item['reference']]
                if op['command'] == 'Move':
                    expected = [copy.deepcopy(item) for item in before if item['reference']] + shifted
                else:
                    expected += shifted
            self.assert_objects_close(value['after'], expected, op['id'])

    def test_native_framebuffers_have_exact_checksums_sizes_and_calibration(self):
        for op, row in self.captures():
            value = row['value']; evidence = value['framebuffer']
            data = (ROOT/'observations'/evidence['path']).read_bytes()
            self.assertEqual(hashlib.sha256(data).hexdigest(), evidence['sha256'], op['id'])
            self.assertEqual(list(struct.unpack('>II', data[16:24])), evidence['size'])
            self.assertEqual(evidence['size'], value['pending']['frame']['size'])
            rect = value['calibration']['rect']
            self.assertEqual([rect[2]-rect[0], rect[3]-rect[1]], evidence['size'])

    @unittest.skipIf(Image is None, 'requires Pillow for native preview pixels')
    def test_native_source_and_target_styles_match_move_copy_and_display_modes(self):
        def neighborhood(bitmap, pixel):
            x, y = map(round, pixel)
            return list(bitmap.crop((x-5,y-5,x+6,y+6)).getdata())
        for op, row in self.captures():
            value = row['value']
            with Image.open(ROOT/'observations'/value['framebuffer']['path']) as bitmap:
                bitmap = bitmap.convert('RGB')
                pixels = neighborhood(bitmap, value['calibration']['regions']['source_point'])
                self.assertEqual(any(r>g+50 and r>b+50 for r,g,b in pixels), op['command']=='Copy', op['id'])
                point = destination(op, value['pending']['frame'])
                base = [3.,0.,0.] if op['placement']=='Normal' else [0.,0.,0.]
                target = [p-b+s for p,b,s in zip(point,base,[3.,3.,0.])]
                matrix = value['pending']['frame']['world_to_screen']
                screen = [sum(row[i]*target[i] for i in range(3))+row[3] for row in matrix]
                pixels = neighborhood(bitmap, [screen[0]/screen[3],screen[1]/screen[3]])
                self.assertTrue(any(r>230 and g>230 and b<80 for r,g,b in pixels), op['id'])
                if op['view']=='Top' and op['placement']=='Free' and op['display_mode']!='Wireframe':
                    # Interior of the original box's roof is empty in every mode.
                    x,y = map(round,value['calibration']['regions']['source_box'])
                    r,g,b = bitmap.getpixel((x,y))
                    self.assertFalse(r>g+40 and r>b+40, op['id'])
                    target=[p-b+s for p,b,s in zip(point,base,[3.25,-4.25,2.])]
                    screen=[sum(row[i]*target[i] for i in range(3))+row[3] for row in matrix]
                    r,g,b=bitmap.getpixel((round(screen[0]/screen[3]),round(screen[1]/screen[3])))
                    self.assertTrue(r>g+10 and r>b+10,op['id'])

    def test_probe_rejects_unbounded_and_mixed_recipes(self):
        case = request()['operations'][0]
        for changes in [dict(id='bad;_Delete'),dict(script='_Delete'),dict(view='Other'),dict(command='Delete'),
                        dict(command='Copy',placement='Normal'),dict(placement='Repeat'),dict(placement='Vertical',view='Top')]:
            with self.assertRaises(ValueError):
                validate_request(dict(protocol_version=1,operations=[dict(case,**changes)]))
        for invalid in [None, [], dict(protocol_version=True,operations=[case]),
                        dict(protocol_version=1,iterations=2,operations=[case]),
                        dict(protocol_version=1,operations=[case,case]),
                        dict(protocol_version=1,operations=[dict(case,id=str(i)) for i in range(49)])]:
            with self.assertRaises(ValueError): validate_request(invalid)

    def test_picker_requires_owned_pid_capture_ack_and_stable_window(self):
        op = request()['operations'][0]
        picker = TranslationPreviewPicker(dict(protocol_version=1,operations=[op]))
        name = '@translation-preview:'+op['id']
        with tempfile.TemporaryDirectory() as folder, patch('tools.rhino_oracle.mirror_preview_input.subprocess.run') as send:
            job=Path(folder);(job/'worker-progress.log').write_text('PICK '+name+' 100 120\n')
            picker.ready[name]=0;picker(job,set());send.assert_not_called()
            picker.images[name]={'captured':True};picker.moved[name]=('owned','100','120',0)
            self.assertFalse(picker.send_input(name,'100','120','owned'));send.assert_not_called()
            (job/('translation-preview-ready-'+op['id']+'.json')).write_text(json.dumps(op['id']))
            with self.assertRaises(OracleProtocolError):picker.send_input(name,'100','120','foreign')
            send.assert_not_called()
            self.assertTrue(picker.send_input(name,'100','120','owned'))
            self.assertEqual(send.call_args.args[0][-2:],['click','1'])
        response=dict(results=[dict(id=op['id'],value={'raw':'preserved'})])
        with self.assertRaises(OracleProtocolError):picker.record_diagnostics(response)
        self.assertEqual(response['results'][0]['value'],{'raw':'preserved'})
        picker.seen.add(name);picker.record_diagnostics(response)
        self.assertEqual(response['results'][0]['value']['raw'],'preserved')

    def test_picker_retries_motion_until_native_snapshot_without_capturing_or_clicking(self):
        op=request()['operations'][0];name='@translation-preview:'+op['id']
        picker=TranslationPreviewPicker(dict(protocol_version=1,operations=[op]))
        picker.moved[name]=('owned','100','120',0)
        with tempfile.TemporaryDirectory() as folder,patch('tools.rhino_oracle.translation_preview_input.subprocess.run') as send:
            picker.job=Path(folder)
            self.assertFalse(picker.send_input(name,'100','120','owned'))
            self.assertEqual(send.call_args.args[0][-6:],['mousemove','101','120','mousemove','100','120'])
            self.assertEqual(picker.images,{})
            with self.assertRaises(OracleProtocolError):picker.send_input(name,'100','120','foreign')
            self.assertEqual(send.call_count,1)

    def test_normal_reference_click_is_required_before_recording_native_results(self):
        op=next(o for o in request()['operations'] if o['placement']=='Normal')
        picker=TranslationPreviewPicker(dict(protocol_version=1,operations=[op]))
        name='@translation-preview:'+op['id'];picker.seen.add(name);picker.images[name]={'captured':True}
        response=dict(results=[dict(id=op['id'],value={'raw':'preserved'})])
        with self.assertRaises(OracleProtocolError):picker.record_diagnostics(response)
        self.assertEqual(response['results'][0]['value'],{'raw':'preserved'})
        picker.seen.add('@translation-reference:'+op['id']);picker.record_diagnostics(response)
        self.assertEqual(response['results'][0]['value']['raw'],'preserved')


if __name__=='__main__': unittest.main()

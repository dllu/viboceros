"""Independent equations and raw private-display evidence for affine prompts."""
import copy
import hashlib
import json
import math
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from PIL import Image
from .affine_preview_probe import request, recipe, validate_request
from .affine_preview_input import AffinePreviewPicker
from .client import OracleProtocolError
from .move_normal_probe import validate_camera
from .translation_input import validate_frame
from .transform_copy_capture import capture

ROOT=Path(__file__).parent

def dot(a,b):return sum(x*y for x,y in zip(a,b))
def cross(a,b):return [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
def unit(a):return [v/math.sqrt(dot(a,a)) for v in a]

def destination(op,frame):
    a,b=frame['ray'];v=[y-x for x,y in zip(a,b)]
    if op['cursor']=='Degenerate':return [0.,0.,0.]
    if op['command']=='Scale1D' and op['phase'] not in ('Direction','ZeroDirection'):
        u=unit(recipe(op)['reference']);d=unit(v);ud=dot(u,d)
        amount=(dot(a,u)-dot(a,d)*ud)/(1.-ud*ud)
        return [amount*x for x in u]
    normal=unit(recipe(op)['axis']) if op['command']=='Rotate3D' else [0.,0.,1.]
    t=-dot(a,normal)/dot(v,normal)
    return [x+t*y for x,y in zip(a,v)]

def mapping(op,target):
    spec=recipe(op);reference=spec['reference'];command=op['command']
    if op['cursor']=='Degenerate':return lambda p:p[:]
    if command in ('Scale','Scale2D'):
        f=math.sqrt(dot(target,target)/dot(reference,reference))
        return lambda p:[p[0]*f,p[1]*f,p[2]*(f if command=='Scale' else 1.)]
    if command=='Scale1D':
        u=unit(target if op['phase'] in ('Direction','ZeroDirection') else reference)
        f=spec['factor'] if op['phase'] in ('Direction','ZeroDirection') else abs(dot(target,u))/math.sqrt(dot(reference,reference))
        return lambda p:[p[i]+(f-1.)*dot(p,u)*u[i] for i in range(3)]
    if command in ('Rotate','Rotate3D'):
        axis=unit(spec['axis']) if command=='Rotate3D' else [0.,0.,1.]
        def projected(p):return unit([p[i]-dot(p,axis)*axis[i] for i in range(3)])
        a,b=projected(reference),projected(target);c=dot(a,b);s=dot(axis,cross(a,b))
        return lambda p:[p[i]*c+cross(axis,p)[i]*s+axis[i]*dot(axis,p)*(1.-c) for i in range(3)]
    a,b=unit(reference),unit(target);s=math.sqrt(dot(cross(a,b),cross(a,b)));c=dot(a,b)
    factor=s/c / math.hypot(a[0],a[1])
    if cross(a,b)[2]<0.:factor=-factor
    u=unit([reference[0],reference[1],0.]);v=[-u[1],u[0],0.]
    return lambda p:[p[i]+factor*dot(p,u)*v[i] for i in range(3)]

def transformed(item,mapper,selected):
    item=copy.deepcopy(item);item['selected']=selected
    key='vertices' if item['vertices'] is not None else 'points'
    item[key]=[mapper(p) for p in item[key]]
    item['bounds']=[[extreme(p[i] for p in item[key]) for i in range(3)] for extreme in (min,max)]
    return item

def rows(items):
    return sorted(items,key=lambda v:(v['type'],v['witness'],v['selected'],v['bounds']))

class AffinePreviewTests(unittest.TestCase):
    def captures(self):
        fixture=json.loads((ROOT/'fixtures/affine_preview.json').read_text())
        observed=json.loads((ROOT/'observations/affine_preview.json').read_text())
        self.assertEqual(fixture,request());validate_request(fixture)
        self.assertEqual(len(fixture['operations']),60)
        self.assertEqual([o['id'] for o in fixture['operations']],[r['id'] for r in observed['results']])
        return zip(fixture['operations'],observed['results'])

    def assert_objects(self,actual,expected,label):
        self.assertEqual(len(actual),len(expected),label)
        for a,b in zip(rows(actual),rows(expected)):
            self.assertEqual((a['type'],a['selected'],a['witness']),(b['type'],b['selected'],b['witness']),label)
            for p,q in zip(a['bounds'],b['bounds']):
                for x,y in zip(p,q):self.assertAlmostEqual(x,y,delta=5e-7 if a['type']=='Brep' else 1e-9,msg=label)
            for key in ('vertices','points'):
                if a[key] is None:self.assertIsNone(b[key]);continue
                remaining=b[key][:]
                for p in a[key]:
                    match=next((q for q in remaining if all(abs(x-y)<1e-9 for x,y in zip(p,q))),None)
                    self.assertIsNotNone(match,label);remaining.remove(match)
                self.assertFalse(remaining,label)

    def test_prescribed_inputs_match_pending_and_completed_geometry(self):
        for op,row in self.captures():
            v=row['value'];pending=v['pending'];label=op['id']
            validate_frame(pending['frame']);validate_camera(pending['camera'],pending['frame'])
            expected=copy.deepcopy(v['before']);self.assertEqual(len(expected),4 if op['cursor']=='Degenerate' else 3)
            if op['phase']=='Repeat':
                expected += [transformed(item,mapping(op,[2.,2.,0.]),False) for item in v['before']]
            self.assert_objects(pending['objects'],expected,label)
            if op['finish']=='Click':
                self.assertTrue(v['success'],label)
                target=mapping(op,destination(op,pending['frame']))
                moved=[transformed(item,target,not op['copy']) for item in v['before']]
                expected=expected+moved if op['copy'] else moved
            else:self.assertFalse(v['success'],label)
            self.assert_objects(v['after'],expected,label)

    def test_framebuffer_checksums_and_sizes(self):
        for op,row in self.captures():
            v=row['value'];e=v['framebuffer'];data=(ROOT/'observations'/e['path']).read_bytes()
            self.assertEqual(hashlib.sha256(data).hexdigest(),e['sha256'],op['id'])
            self.assertEqual(list(struct.unpack('>II',data[16:24])),e['size'])
            self.assertEqual(e['size'],v['pending']['frame']['size'])
            r=v['calibration']['rect'];self.assertEqual([r[2]-r[0],r[3]-r[1]],e['size'])

    def test_native_target_point_and_original_source_styles(self):
        for op,row in self.captures():
            v=row['value'];target=mapping(op,destination(op,v['pending']['frame']))([3.,3.,0.])
            matrix=v['pending']['frame']['world_to_screen'];screen=[dot(r[:3],target)+r[3] for r in matrix]
            with Image.open(ROOT/'observations'/v['framebuffer']['path']) as bitmap:
                bitmap=bitmap.convert('RGB')
                def pixels(p):
                    x,y=map(round,p);return list(bitmap.crop((x-5,y-5,x+6,y+6)).getdata())
                self.assertTrue(any(r>230 and g>230 and b<80 for r,g,b in pixels([screen[0]/screen[3],screen[1]/screen[3]])),op['id'])
                # Shaded target faces can overlap the source marker (for example Rotate).
                if op['cursor']!='Degenerate' and op['display_mode']=='Wireframe':
                    self.assertEqual(any(r>g+50 and r>b+50 for r,g,b in pixels(v['calibration']['regions']['source_point'])),op['copy'],op['id'])

    def test_typed_scale1d_projection_capture_is_raw_and_axis_constrained(self):
        f=json.loads((ROOT/'fixtures/scale1d_projection.json').read_text());r=json.loads((ROOT/'observations/scale1d_projection.json').read_text())
        self.assertEqual(capture(f,Mock(settings_scheme='VibocerosOracleTest',run_rhino=Mock(return_value=r))),r)
        self.assertEqual(len(r['results']),6)
        for op,row in zip(f['operations'],r['results']):
            reference=[float(x) for x in op['inputs'][2][1:].split(',')];target=[float(x) for x in op['inputs'][3][1:].split(',')]
            u=unit(reference);factor=abs(dot(u,target))/math.sqrt(dot(reference,reference))
            self.assertEqual(row['value']['succeeded'],factor>0.)
            for item in row['value']['after']['objects']:
                p=op['sources'][item['source']];expected=[p[i]+(factor-1)*dot(p,u)*u[i] for i in range(3)] if factor>0 else p
                for a,b in zip(item['point'],expected):self.assertAlmostEqual(a,b,delta=1e-12)

    def test_probe_rejects_invalid_and_unbounded_recipes(self):
        case=request()['operations'][0]
        for changes in [dict(id='bad;Delete'),dict(script='_Delete'),dict(command='Delete'),dict(copy=1),dict(phase='Repeat'),dict(phase='Direction'),dict(view='Unknown'),dict(cursor='Degenerate'),dict(display_mode='Other')]:
            with self.assertRaises(ValueError):validate_request(dict(protocol_version=1,operations=[dict(case,**changes)]))
        for invalid in [None,[],dict(protocol_version=True,operations=[case]),dict(protocol_version=1,iterations=2,operations=[case]),dict(protocol_version=1,operations=[case,case]),dict(protocol_version=1,operations=[dict(case,id=str(i)) for i in range(65)])]:
            with self.assertRaises(ValueError):validate_request(invalid)

    def test_picker_requires_snapshot_ack_and_owned_window(self):
        op=request()['operations'][0];name='@affine-preview:'+op['id'];picker=AffinePreviewPicker(dict(protocol_version=1,operations=[op]));picker.moved[name]=('owned','100','120',0)
        with tempfile.TemporaryDirectory() as folder,patch('tools.rhino_oracle.affine_preview_input.subprocess.run') as send:
            picker.job=Path(folder);self.assertFalse(picker.send_input(name,'100','120','owned'))
            self.assertEqual(send.call_args.args[0][-6:],['mousemove','101','120','mousemove','100','120'])
            self.assertEqual(picker.images,{})
            with self.assertRaises(OracleProtocolError):picker.send_input(name,'100','120','foreign')
            self.assertEqual(send.call_count,1)
        response=dict(results=[dict(id=op['id'],value={'raw':'preserved'})])
        with self.assertRaises(OracleProtocolError):picker.record_diagnostics(response)
        self.assertEqual(response['results'][0]['value'],{'raw':'preserved'})

if __name__=='__main__':unittest.main()

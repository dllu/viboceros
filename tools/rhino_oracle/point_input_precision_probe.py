"""Public Point parsing at double neighbors of the float-epsilon origin."""
import json
import re
import struct

if globals().get('__package__'):
    from .number_token import number_token
else:
    from number_token import number_token

ORIGINS = dict(Zero=[0.,0.,0.],FloatBelow=[1.1920928955078124e-7,0.,0.],
               FloatAt=[2.**-23,0.,0.],FloatAbove=[1.1920928955078128e-7,0.,0.],
               FloatHigh=[2.**-23+2.**-43,0.,0.],NegativeAbove=[-1.1920928955078128e-7,0.,0.],
               YAbove=[0.,1.1920928955078128e-7,0.],ZAbove=[0.,0.,1.1920928955078128e-7])


def validate_request(q):
    if (not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1
            or not isinstance(q.get('operations'),list) or not 1 <= len(q['operations']) <= 64):
        raise ValueError('point input precision requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','origin','format','prime'}
                or op['op']!='point_input_precision' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,70}\Z',op['id']) is None or op['id'] in seen
                or op['origin'] not in tuple(ORIGINS)
                or op['format'] not in ('RoundTrip','Scientific','Fixed','Fraction')
                or op['prime'] not in ('None','ScalePositions')):
            raise ValueError('invalid point input precision recipe')
        seen.add(op['id'])


def bits(value):
    return '%016x' % struct.unpack('!Q',struct.pack('!d',float(value)))[0]


def component(value,style):
    if value==0.: return '0'
    token = number_token(value)
    if style=='RoundTrip': return token
    digits,exponent = token.split('e'); exponent = int(exponent)
    sign = '-' if digits.startswith('-') else ''
    digits = digits.lstrip('-')
    if style=='Scientific': return sign+digits[0]+'.'+digits[1:]+'e'+str(exponent+len(digits)-1)
    if style=='Fixed':
        split = len(digits)+exponent
        return sign+('0.'+'0'*(-split)+digits if split <= 0 else digits[:split]+'.'+digits[split:])
    raw = struct.unpack('!Q',struct.pack('!d',float(value)))[0]
    mantissa = (raw & ((1<<52)-1))+(1<<52)
    binary_exponent = ((raw>>52)&2047)-1023-52
    return sign+str(mantissa)+'/'+str(1<<(-binary_exponent))


def recipe(op):
    point = ORIGINS[op['origin']][:]
    token = 'w'+','.join(component(x,op['format']) for x in point)
    return dict(point=point,bits=[bits(x) for x in point],token=token,
                macro='_Point '+token+' _Cancel',
                seed_macro='_ScalePositions _Copy=_No _Mode=_1D w0,0,0 w1,0,0 w2,0,0' if op['prime']=='ScalePositions' else None)


def request():
    rows = [(origin,style,prime) for origin in ORIGINS for style in ('RoundTrip','Scientific','Fixed','Fraction')
            for prime in ('None','ScalePositions')]
    return dict(protocol_version=1,iterations=1,operations=[dict(op='point_input_precision',
        id='point-input-precision-'+str(i),origin=origin,format=style,prime=prime)
        for i,(origin,style,prime) in enumerate(rows)])


def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino = host['Rhino']
    doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand(): raise ValueError('point input precision requires idle execution')
    if list(doc.Objects): raise ValueError('point input precision requires an empty owned document')
    from join_probe import observe_command
    spec = recipe(op)
    view = doc.Views.ActiveView; vp = view.ActiveViewport
    old_view = Rhino.DocObjects.ViewportInfo(vp); old_plane = vp.GetConstructionPlane()
    def record(p):
        xyz = host['_xyz'](p)
        return dict(point=xyz,bits=[bits(x) for x in xyz],tiny=Rhino.Geometry.Vector3d(p).IsTiny(2.**-23))
    def objects():
        return [record(obj.Geometry.Location) for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber)]
    try:
        if not vp.SetProjection(Rhino.Display.DefinedViewportProjection.Top,'Owned point input precision',False):
            raise ValueError('point precision view setup failed')
        vp.SetConstructionPlane(Rhino.Geometry.Plane.WorldXY)
        if spec['seed_macro']:
            seed = doc.Objects.AddPoint(host['_point']([1.,1.,1.])); doc.Objects.Select(seed)
            if not Rhino.RhinoApp.RunScript(spec['seed_macro'],True) or Rhino.Commands.Command.InCommand():
                raise ValueError('point precision seed failed')
            for obj in list(doc.Objects): doc.Objects.Delete(obj.Id,True)
        history_marker = 'Viboceros point input precision '+str(host['System'].Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(history_marker)
        success,after,events = observe_command(Rhino.Commands.Command,'Point',
            lambda:Rhino.RhinoApp.RunScript(spec['macro'],True),objects,lambda:[],True)
        if Rhino.Commands.Command.InCommand(): raise ValueError('point precision macro remains active')
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(history_marker,1)[-1]
        return dict(recipe=spec,success=success,after=after,after_script=objects(),events=events,
                    history=history,tolerance=doc.ModelAbsoluteTolerance),0
    finally:
        for obj in list(doc.Objects): doc.Objects.Delete(obj.Id,True)
        vp.SetViewProjection(old_view,False); vp.SetConstructionPlane(old_plane)
        old_view.Dispose(); doc.Views.Redraw()

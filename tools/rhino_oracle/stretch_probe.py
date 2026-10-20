"""Bounded public StretchSpaceMorph witnesses and owned Stretch commands."""
import math
import re


def validate(op):
    if not isinstance(op, dict) or op.get('op') not in ('stretch_sdk', 'stretch_command'):
        raise ValueError('invalid Stretch operation')
    common = {'op', 'id', 'start', 'end', 'points'}
    choices = {'length', 'target'} if op['op'] == 'stretch_sdk' else {'factor', 'length', 'target'}
    values = choices.intersection(op)
    extra = {'copy', 'rigid', 'grouped'} if op['op'] == 'stretch_command' else set()
    optional={'preselect'} if op['op']=='stretch_command' else set()
    if len(values) != 1 or set(op)-optional != common | values | extra:
        raise ValueError('invalid Stretch recipe keys')
    if not isinstance(op['id'], str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None:
        raise ValueError('invalid Stretch identity')
    def number(v):return type(v) in (int, float) and not math.isnan(v) and not math.isinf(v) and abs(v) <= 1e6
    if not isinstance(op['points'], list) or not 1 <= len(op['points']) <= 256:
        raise ValueError('invalid Stretch sample count')
    for p in [op['start'], op['end']] + op['points'] + ([op['target']] if 'target' in op else []):
        if not isinstance(p, list) or len(p) != 3 or not all(number(v) for v in p):raise ValueError('invalid Stretch point')
    for key in ('factor', 'length'):
        if key in op and not number(op[key]):raise ValueError('invalid Stretch scalar')
    if type(op.get('preselect',True)) is not bool:raise ValueError('invalid Stretch preselection')
    if any(type(op[k]) is not bool for k in extra):raise ValueError('invalid Stretch options')


def run(op, host, iterations, tolerance):
    validate(op)
    if op['op'] == 'stretch_command':return command(op, host)
    G = host['Rhino'].Geometry
    value = host['_point'](op['target']) if 'target' in op else float(op['length'])
    morph = G.Morphs.StretchSpaceMorph(host['_point'](op['start']), host['_point'](op['end']), value)
    try:
        points = [host['_point'](p) for p in op['points']]
        result, elapsed = host['_measure'](iterations, lambda: [host['_xyz'](morph.MorphPoint(p)) for p in points])
        return dict(valid=bool(morph.IsValid), points=result), elapsed
    finally:morph.Dispose()


def command(op, host):
    from join_probe import observe_command
    R, S = host['Rhino'], host['System'];doc = R.RhinoDoc.ActiveDoc
    if list(doc.Objects):raise ValueError('Stretch requires an empty owned document')
    ids = [];saved_tolerance = doc.ModelAbsoluteTolerance
    def snapshot():
        return [dict(source=ids.index(o.Id) if o.Id in ids else None, selected=bool(o.IsSelected(False)), point=host['_xyz'](o.Geometry.Location)) for o in sorted(list(doc.Objects), key=lambda o:o.RuntimeSerialNumber)]
    def point(p):return 'w'+','.join('%.17g'%v for v in p)
    try:
        doc.ModelAbsoluteTolerance = 1e-6
        record = doc.BeginUndoRecord('Owned Stretch sources')
        try:
            for p in op['points']:ids.append(doc.Objects.AddPoint(host['_point'](p)))
            if any(i == S.Guid.Empty for i in ids):raise ValueError('Stretch source insertion failed')
            if op['grouped']:doc.Groups.Add('OwnedStretch', ids)
            if op.get('preselect',True):
                for i in ids:doc.Objects.Select(i)
        finally:doc.EndUndoRecord(record)
        before = snapshot()
        options = ' '.join('_'+k+'='+('Yes'if op[k.lower()]else'No') for k in ('Copy','Rigid'))
        value = point(op['target'])if 'target'in op else '_Length %.17g'%op['length']if 'length'in op else '%.17g'%op['factor']
        macro = '_Stretch '+(''if op.get('preselect',True)else'_SelAll _Enter ')+point(op['start'])+' '+point(op['end'])+' '+options+' '+value+(' _Cancel'if op.get('factor')==1. else' _Enter'if op['copy']else'')
        marker = 'Viboceros Stretch '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' '+macro)
        success, after, events = observe_command(R.Commands.Command,'Stretch',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        history = R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();undo=redo=None
        if success:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,before=before,after=after,undo=undo,redo=redo,events=events,history=history,macro=macro),0
    finally:
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for i in range(doc.Groups.Count):
            if not doc.Groups[i].IsDeleted:doc.Groups.Delete(i)
        doc.ModelAbsoluteTolerance = saved_tolerance

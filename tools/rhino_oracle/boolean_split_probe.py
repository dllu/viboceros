"""Owned public BooleanSplit commands; closed recipes and complete metadata."""
import re

CUBE = lambda lo, hi: ((lo, hi),) * 3
SPECS = {
    'slab': dict(bounds=[CUBE(0.,2.), ((.75,1.25),(-1.,3.),(-1.,3.))]),
    'corner': dict(bounds=[CUBE(0.,2.), CUBE(1.,3.)]),
    'contained': dict(bounds=[CUBE(0.,2.), CUBE(.5,1.5)]),
    'contains': dict(bounds=[CUBE(0.,2.), CUBE(-1.,3.)]),
    'equal': dict(bounds=[CUBE(0.,2.), CUBE(0.,2.)]),
    'touch': dict(bounds=[CUBE(0.,2.), ((2.,4.),(0.,2.),(0.,2.))]),
    'edge_touch': dict(bounds=[CUBE(0.,2.), ((2.,4.),(2.,4.),(0.,2.))]),
    'point_touch': dict(bounds=[CUBE(0.,2.), CUBE(2.,4.)]),
    'disjoint': dict(bounds=[CUBE(0.,2.), CUBE(4.,5.)]),
    'two_slabs': dict(bounds=[CUBE(0.,3.), ((.5,1.),(-1.,4.),(-1.,4.)), ((2.,2.5),(-1.,4.),(-1.,4.))]),
    'cross_xy': dict(bounds=[CUBE(0.,4.), ((1.,3.),(-1.,5.),(-1.,5.)), ((-1.,5.),(1.,3.),(-1.,5.))]),
    'overlap_cutters': dict(bounds=[CUBE(0.,3.), CUBE(1.,4.), CUBE(2.,5.)]),
    'disjoint_target': dict(bounds=[CUBE(0.,2.), CUBE(10.,11.), CUBE(1.,3.)], first=[0,1], second=[2]),
    'overlap_targets': dict(bounds=[CUBE(0.,3.), CUBE(1.,4.), CUBE(2.,5.)], first=[0,1], second=[2]),
    'shared_sets': dict(bounds=[CUBE(0.,2.), CUBE(1.,3.)], first=[0,1], second=[0,1]),
}
for case, flags in (
    ('slab_keep', dict(delete=False)), ('slab_pre', dict(pre=True)),
    ('slab_pre_keep', dict(pre=True, delete=False)),
    ('slab_cancel', dict(cancel='second')),
    ('slab_cancel_options', dict(cancel='first', delete=False)),
):
    SPECS[case] = dict(SPECS['slab'], **flags)
SPECS['cross_xy_reverse'] = dict(SPECS['cross_xy'], second=[2,1])
SPECS['two_slabs_reverse'] = dict(SPECS['two_slabs'], second=[2,1])
SPECS['corner_reverse'] = dict(SPECS['corner'], first=[1], second=[0])
SPECS['boundary_contained'] = dict(bounds=[CUBE(0.,2.),CUBE(0.,1.)])
SPECS['duplicate_cutters'] = dict(bounds=[CUBE(0.,2.),((.75,1.25),(-1.,3.),(-1.,3.)),((.75,1.25),(-1.,3.),(-1.,3.))])
SPECS['slab_pre_disjoint'] = dict(SPECS['disjoint'], pre=True)
SPECS['internal_and_cutting'] = dict(bounds=[CUBE(0.,4.), CUBE(.5,1.5), CUBE(3.,5.)])
SPECS['slab_cancel_cutters_options'] = dict(SPECS['slab'], cancel='second', second_delete=False)
CASES = tuple(SPECS)


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='boolean_split_command', id='boolean_split_'+c, case=c) for c in CASES])


def validate_request(q):
    if (not isinstance(q, dict) or set(q) - {'protocol_version','iterations','operations'}
            or type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1) != 1
            or not isinstance(q.get('operations'),list) or not 1 <= len(q['operations']) <= 64):
        raise ValueError('BooleanSplit requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op) != {'op','id','case'}
                or op['op'] != 'boolean_split_command' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
                or op['id'] in seen or not isinstance(op['case'],str) or op['case'] not in SPECS):
            raise ValueError('invalid BooleanSplit recipe')
        seen.add(op['id'])


def run(op, host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    G, doc = Rhino.Geometry, Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():
        raise ValueError('BooleanSplit requires idle execution')
    if list(doc.Objects):
        raise ValueError('BooleanSplit requires an empty owned document')
    from join_probe import observe_command
    ids, layers, groups = [], [], []
    saved_layer, saved_tolerance = doc.Layers.CurrentLayerIndex, doc.ModelAbsoluteTolerance
    doc.ModelAbsoluteTolerance = 1e-7
    spec = SPECS[op['case']]
    first = spec.get('first',[0]); second = spec.get('second',list(range(1,len(spec['bounds']))))

    def box(bounds):
        return G.BoundingBox(G.Point3d(*[p[0] for p in bounds]),G.Point3d(*[p[1] for p in bounds])).ToBrep()

    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g,a=obj.Geometry,obj.Attributes
            area=G.AreaMassProperties.Compute(g)
            mass=G.VolumeMassProperties.Compute(g) if g.IsSolid else None
            try:
                rows.append(dict(source=ids.index(obj.Id) if obj.Id in ids else None,
                    selected=bool(obj.IsSelected(False)),kind='Brep',name=a.Name,
                    layer=layers.index(a.LayerIndex) if a.LayerIndex in layers else None,
                    color=[int(a.ObjectColor.R),int(a.ObjectColor.G),int(a.ObjectColor.B)],
                    groups=sorted(groups.index(i) for i in (a.GetGroupList() or []) if i in groups),
                    attribute_text=a.GetUserString('Code'),geometry_text=g.GetUserString('Code'),
                    valid=bool(g.IsValid),solid=bool(g.IsSolid),faces=g.Faces.Count,edges=g.Edges.Count,
                    volume=float(mass.Volume) if mass else None,area=float(area.Area) if area else None,
                    centroid=host['_xyz'](mass.Centroid) if mass else None,
                    vertices=[host['_xyz'](v.Location) for v in g.Vertices],
                    face_regions=[[[host['_xyz'](f.PointAt(t.PointAtStart.X,t.PointAtStart.Y))
                        for t in loop.Trims] for loop in f.Loops] for f in g.Faces]))
            finally:
                if area:area.Dispose()
                if mass:mass.Dispose()
        return rows

    def invoke(macro, command):
        marker='Viboceros BooleanSplit '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' '+macro)
        success,after,events=observe_command(Rhino.Commands.Command,command,
            lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        if Rhino.Commands.Command.InCommand():
            Rhino.RhinoApp.RunScript('!',False)
            raise ValueError('BooleanSplit remains active')
        return dict(success=success,macro=macro,after=after,after_script=snapshot(),events=events,
            history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1])

    try:
        for i,bounds in enumerate(spec['bounds']):
            layer=Rhino.DocObjects.Layer();layer.Name='Viboceros BooleanSplit '+str(System.Guid.NewGuid())
            try:layers.append(doc.Layers.Add(layer))
            finally:layer.Dispose()
            g=box(bounds);a=Rhino.DocObjects.ObjectAttributes()
            try:
                a.LayerIndex,a.Name=layers[i],'source-'+str(i)
                a.ObjectColor=System.Drawing.Color.FromArgb(20+i,40,60)
                a.ColorSource=Rhino.DocObjects.ObjectColorSource.ColorFromObject
                a.SetUserString('Code','attribute-'+str(i));g.SetUserString('Code','geometry-'+str(i))
                ids.append(doc.Objects.AddBrep(g,a))
                if ids[-1]==System.Guid.Empty:raise ValueError('BooleanSplit source insertion failed')
            finally:g.Dispose();a.Dispose()
        for members in [[key] for key in ids]+[ids]:
            groups.append(doc.Groups.Add('Viboceros BooleanSplit '+str(System.Guid.NewGuid()),members))
        doc.ClearUndoRecords(True)
        if spec.get('pre'):
            for i in first:doc.Objects.Select(ids[i])
        before=snapshot()
        select=lambda indices:' '.join('_SelID '+str(ids[i]) for i in indices)
        options='_DeleteInput=_'+('Yes' if spec.get('delete',True) else 'No')+' '
        macro='_-BooleanSplit '+options
        if spec.get('cancel')=='first':macro+='_Cancel'
        else:
            if not spec.get('pre'):macro+=select(first)+' _Enter '
            if 'second_delete' in spec:macro+='_DeleteInput=_'+('Yes' if spec['second_delete'] else 'No')+' '
            macro+=select(second)+(' _Cancel' if spec.get('cancel')=='second' else ' _Enter')
        result=dict(case=op['case'],bounds=spec['bounds'],first=first,second=second,
            pre=spec.get('pre',False),delete=spec.get('delete',True),before=before,command=invoke(macro,'BooleanSplit'))
        result['undo']=invoke('_Undo','Undo');result['redo']=invoke('_Redo','Redo')
        if op['case'] in ('slab_cancel_options','slab_cancel_cutters_options'):
            result['followup_before']=snapshot()
            result['followup']=invoke('_-BooleanSplit '+select(first)+' _Enter '+select(second)+' _Enter','BooleanSplit')
        return result,0
    finally:
        if Rhino.Commands.Command.InCommand():Rhino.RhinoApp.RunScript('!',False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for index in groups:doc.Groups.Delete(index)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for index in reversed(layers):doc.Layers.Delete(index,True)
        doc.ModelAbsoluteTolerance=saved_tolerance

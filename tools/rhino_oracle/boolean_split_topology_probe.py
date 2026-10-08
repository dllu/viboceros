"""Owned BooleanSplit finite plane cutters with closed bounded recipe shapes."""
import re

def cube(lo, hi):return dict(kind='box', bounds=((lo,hi),)*3)
def plane_x(x,lo=-1.,hi=3.,zlo=-1.,zhi=3.):
    return dict(kind='plane',points=[(x,lo,zlo),(x,hi,zlo),(x,hi,zhi),(x,lo,zhi)])
def plane_y(y,lo=-1.,hi=3.):
    return dict(kind='plane',points=[(lo,y,-1.),(lo,y,3.),(hi,y,3.),(hi,y,-1.)])
def plane_z(z):
    return dict(kind='plane',points=[(-1.,-1.,z),(3.,-1.,z),(3.,3.,z),(-1.,3.,z)])
def compound(parts):return dict(kind='compound',parts=parts)
def sheet_hole(hole):return dict(kind='sheet_hole',outer=[(1.,-1.,-1.),(1.,3.,-1.),(1.,3.,3.),(1.,-1.,3.)],hole=[(1.,hole[0],hole[0]),(1.,hole[1],hole[0]),(1.,hole[1],hole[1]),(1.,hole[0],hole[1])])
SPECS = {
    'compound_target_one_shell':dict(shapes=[compound([cube(0.,2.),cube(4.,6.)]),plane_x(1.)]),
    'compound_target_both_shells':dict(shapes=[compound([cube(0.,2.),dict(kind='box',bounds=((0.,2.),(4.,6.),(4.,6.)))]),plane_x(1.,-1.,7.,-1.,7.)]),
    'compound_cutter_one_shell':dict(shapes=[cube(0.,2.),compound([cube(1.,3.),cube(4.,6.)])]),
    'compound_cutter_two_slabs':dict(shapes=[cube(0.,3.),compound([dict(kind='box',bounds=((.5,1.),(-1.,4.),(-1.,4.))),dict(kind='box',bounds=((2.,2.5),(-1.,4.),(-1.,4.)))])]),
    'compound_open_target':dict(shapes=[compound([plane_x(1.),plane_x(1.,4.,6.,4.,6.)]),cube(0.,2.)]),
    'sheet_hole_cross_section':dict(shapes=[cube(0.,2.),sheet_hole((.5,1.5))]),
    'sheet_hole_outside_section':dict(shapes=[dict(kind='box',bounds=((0.,2.),(1.,2.),(1.,2.))),sheet_hole((-.5,.5))]),
    'open_target_hole_then_box':dict(shapes=[sheet_hole((.5,1.5)),cube(0.,2.)]),
    'open_target_hole_then_plane':dict(shapes=[sheet_hole((.5,1.5)),plane_y(1.)]),
    'open_target_hole_then_coplanar_fill':dict(shapes=[sheet_hole((.5,1.5)),plane_x(1.,.5,1.5,.5,1.5)]),
    'open_target_hole_then_coplanar_cover':dict(shapes=[sheet_hole((.5,1.5)),plane_x(1.,0.,2.,0.,2.)]),
    'open_target_hole_then_coplanar_straddle':dict(shapes=[sheet_hole((.5,1.5)),plane_x(1.,0.,1.,0.,1.)]),
}
SPECS['compound_keep'] = dict(SPECS['compound_target_one_shell'],delete=False)
SPECS['compound_pre'] = dict(SPECS['compound_target_one_shell'],pre=True)
CASES = tuple(SPECS)


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='boolean_split_topology', id='split_topology_'+c, case=c) for c in CASES])


def validate_request(q):
    if (not isinstance(q, dict) or set(q) - {'protocol_version','iterations','operations'}
            or type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1) != 1
            or not isinstance(q.get('operations'),list) or not 1 <= len(q['operations']) <= 64):
        raise ValueError('BooleanSplit requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op) != {'op','id','case'}
                or op['op'] != 'boolean_split_topology' or not isinstance(op['id'],str)
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
    first = spec.get('first',[0]); second = spec.get('second',list(range(1,len(spec['shapes']))))

    def box(bounds):
        return G.BoundingBox(G.Point3d(*[p[0] for p in bounds]),G.Point3d(*[p[1] for p in bounds])).ToBrep()

    def geometry(shape):
        if shape['kind']=='box':return box(shape['bounds'])
        if shape['kind']=='compound':
            result=G.Brep()
            for part in shape['parts']:
                g=geometry(part)
                try:result.Append(g)
                finally:g.Dispose()
            return result
        if shape['kind']=='sheet_hole':
            curves=[]
            for points in [shape['outer'],shape['hole']]:curves.append(G.PolylineCurve([G.Point3d(*p) for p in points+[points[0]]]))
            try:
                result=G.Brep.CreatePlanarBreps(curves,1e-7)
                if result is None or len(result)!=1:raise ValueError('owned planar ring construction failed')
                return result[0]
            finally:
                for c in curves:c.Dispose()
        surface=G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p) for p in shape['points']])
        try:g=surface.ToBrep()
        finally:surface.Dispose()
        if shape.get('reverse'):g.Flip()
        return g

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
        for i,shape in enumerate(spec['shapes']):
            layer=Rhino.DocObjects.Layer();layer.Name='Viboceros BooleanSplit '+str(System.Guid.NewGuid())
            try:layers.append(doc.Layers.Add(layer))
            finally:layer.Dispose()
            g=geometry(shape)
            a=Rhino.DocObjects.ObjectAttributes()
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
        result=dict(case=op['case'],shapes=spec['shapes'],first=first,second=second,
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

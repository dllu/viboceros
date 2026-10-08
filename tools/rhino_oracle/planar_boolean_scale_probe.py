"""Owned public PlanarBoolean commands; coplanar recipes and complete metadata."""
import re

def disk(scale):return dict(kind='disk',center=[0.,0.,0.],radius=2.*scale,normal=[0.,0.,1.])
def rectangle(lo,hi,scale):return dict(kind='plane',points=[[lo*scale,-3.*scale,0.],[hi*scale,-3.*scale,0.],[hi*scale,3.*scale,0.],[lo*scale,3.*scale,0.]])
SPECS={}
for command in ('PlanarUnion','PlanarDifference','PlanarIntersection'):
    for exponent in (-20,0,20):
        scale=2.**exponent
        for family,shapes in {'side':[disk(scale),rectangle(0.,3.,scale)],'strip':[disk(scale),rectangle(-.5,.5,scale)],'first_polygon':[rectangle(0.,3.,scale),disk(scale)]}.items():
            SPECS[command.lower()+'_'+family+'_'+str(exponent).replace('-','m')]=dict(command=command,shapes=shapes,scale=scale)
CASES = tuple(SPECS)


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='planar_boolean_scale', id='planar_boolean_scale_'+c, case=c) for c in CASES])


def validate_request(q):
    if (not isinstance(q, dict) or set(q) - {'protocol_version','iterations','operations'}
            or type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1) != 1
            or not isinstance(q.get('operations'),list) or not 1 <= len(q['operations']) <= 64):
        raise ValueError('PlanarBoolean requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op) != {'op','id','case'}
                or op['op'] != 'planar_boolean_scale' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
                or op['id'] in seen or not isinstance(op['case'],str) or op['case'] not in SPECS):
            raise ValueError('invalid PlanarBoolean recipe')
        seen.add(op['id'])


def run(op, host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    G, doc = Rhino.Geometry, Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():
        raise ValueError('PlanarBoolean requires idle execution')
    if list(doc.Objects):
        raise ValueError('PlanarBoolean requires an empty owned document')
    from join_probe import observe_command
    import os
    root=os.path.dirname(os.path.abspath(host['__file__']))
    ids, layers, groups = [], [], []
    saved_layer, saved_tolerance = doc.Layers.CurrentLayerIndex, doc.ModelAbsoluteTolerance
    spec = SPECS[op['case']]
    doc.ModelAbsoluteTolerance = 1e-7*spec['scale']

    def box(bounds):
        return G.BoundingBox(G.Point3d(*[p[0] for p in bounds]),G.Point3d(*[p[1] for p in bounds])).ToBrep()

    def geometry(shape):
        curves=[]
        if shape['kind']=='plane':
            surface=G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p)for p in shape['points']])
            try:return surface.ToBrep()
            finally:surface.Dispose()
        if shape['kind']=='sheet_hole':
            curves=[G.PolylineCurve([G.Point3d(*p)for p in points+[points[0]]])for points in [shape['outer'],shape['hole']]]
        else:
            plane=G.Plane(G.Point3d(*shape['center']),G.Vector3d(*shape['normal']))
            circle=G.Circle(plane,shape['radius'])
            if shape['kind']=='half_disk':
                arc=G.Arc(circle,System.Math.PI)
                curves=[G.ArcCurve(arc),G.LineCurve(arc.EndPoint,arc.StartPoint)]
            else:
                curves=[circle.ToNurbsCurve()]
                if shape['kind']=='annulus':curves.append(G.Circle(plane,shape['inner']).ToNurbsCurve())
        try:
            result=G.Brep.CreatePlanarBreps(curves,doc.ModelAbsoluteTolerance)
            if result is None or len(result)!=1:raise ValueError('owned mixed planar construction failed')
            g=result[0]
        finally:
            for curve in curves:curve.Dispose()
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
                    edge_curves=[host['_nurbs_curve_definition'](e)for e in g.Edges],
                    edge_samples=[[host['_xyz'](e.PointAt(e.Domain.ParameterAt(float(i)/32.)))for i in range(33)]for e in g.Edges],
                    face_regions=[[[host['_xyz'](f.PointAt(t.PointAtStart.X,t.PointAtStart.Y))
                        for t in loop.Trims] for loop in f.Loops] for f in g.Faces]))
            finally:
                if area:area.Dispose()
                if mass:mass.Dispose()
        return rows

    def invoke(macro, command):
        marker='Viboceros PlanarBoolean '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' '+macro)
        success,after,events=observe_command(Rhino.Commands.Command,command,
            lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        if Rhino.Commands.Command.InCommand():
            Rhino.RhinoApp.RunScript('!',False)
            raise ValueError('PlanarBoolean remains active')
        return dict(success=success,macro=macro,after=after,after_script=snapshot(),events=events,
            history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1])

    try:
        for i,shape in enumerate(spec['shapes']):
            layer=Rhino.DocObjects.Layer();layer.Name='Viboceros PlanarBoolean '+str(System.Guid.NewGuid())
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
                if ids[-1]==System.Guid.Empty:raise ValueError('PlanarBoolean source insertion failed')
            finally:g.Dispose();a.Dispose()
        for members in [[key] for key in ids]+[ids]:
            groups.append(doc.Groups.Add('Viboceros PlanarBoolean '+str(System.Guid.NewGuid()),members))
        doc.ClearUndoRecords(True)
        if spec.get('pre'):
            for key in ids:doc.Objects.Select(key)
        before=snapshot()
        select=' '.join('_SelID '+str(key) for key in ids)
        macro='_'+spec['command']+('' if spec.get('pre') and spec['command']!='PlanarDifference' else ' '+select)+(' _Enter' if spec['command']=='PlanarUnion' else '')
        result=dict(case=op['case'],shapes=spec['shapes'],command_name=spec['command'],pre=spec.get('pre',False),scale=spec['scale'],before=before,command=invoke(macro,spec['command']))
        result['undo']=invoke('_Undo','Undo');result['redo']=invoke('_Redo','Redo')
        return result,0
    finally:
        if Rhino.Commands.Command.InCommand():Rhino.RhinoApp.RunScript('!',False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for index in groups:doc.Groups.Delete(index)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for index in reversed(layers):doc.Layers.Delete(index,True)
        doc.ModelAbsoluteTolerance=saved_tolerance

"""Owned public Boolean2Objects commands; closed recipes and complete metadata."""
import re

CUBE=lambda lo,hi:((lo,hi),)*3
SPECS = { 'cycle_'+str(i):dict(bounds=[CUBE(0.,2.),CUBE(1.,3.)],cycles=i) for i in range(6) }
SPECS['keep']=dict(SPECS['cycle_0'],delete=False)
SPECS['cancel']=dict(SPECS['cycle_2'],cancel=True)
SPECS['disjoint'] = dict(bounds=[CUBE(0.,2.),CUBE(4.,5.)],cycles=0)
SPECS['contained'] = dict(bounds=[CUBE(0.,3.),CUBE(1.,2.)],cycles=0)
SPECS['contained_cycle_3'] = dict(SPECS['contained'],cycles=3)
CASES = tuple(SPECS)


def request():
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op='boolean_two_command', id='boolean_two_'+c, case=c) for c in CASES])


def validate_request(q):
    if (not isinstance(q, dict) or set(q) - {'protocol_version','iterations','operations'}
            or type(q.get('protocol_version')) is not int or q['protocol_version'] != 1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1) != 1
            or not isinstance(q.get('operations'),list) or not 1 <= len(q['operations']) <= 64):
        raise ValueError('Boolean2Objects requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op) != {'op','id','case'}
                or op['op'] != 'boolean_two_command' or not isinstance(op['id'],str)
                or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
                or op['id'] in seen or not isinstance(op['case'],str) or op['case'] not in SPECS):
            raise ValueError('invalid Boolean2Objects recipe')
        seen.add(op['id'])


def run(op, host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino, System = host['Rhino'], host['System']
    G, doc = Rhino.Geometry, Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():
        raise ValueError('Boolean2Objects requires idle execution')
    if list(doc.Objects):
        raise ValueError('Boolean2Objects requires an empty owned document')
    from join_probe import observe_command
    import clr,json,os,time
    clr.AddReference('System.Windows.Forms')
    root=os.path.dirname(os.path.abspath(host['__file__']))
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
        marker='Viboceros Boolean2Objects '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker);host['_record_progress'](op['id']+' '+macro)
        success,after,events=observe_command(Rhino.Commands.Command,command,
            lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        if Rhino.Commands.Command.InCommand():
            Rhino.RhinoApp.RunScript('!',False)
            raise ValueError('Boolean2Objects remains active')
        return dict(success=success,macro=macro,after=after,after_script=snapshot(),events=events,
            history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1])

    try:
        for i,bounds in enumerate(spec['bounds']):
            layer=Rhino.DocObjects.Layer();layer.Name='Viboceros Boolean2Objects '+str(System.Guid.NewGuid())
            try:layers.append(doc.Layers.Add(layer))
            finally:layer.Dispose()
            g=box(bounds);a=Rhino.DocObjects.ObjectAttributes()
            try:
                a.LayerIndex,a.Name=layers[i],'source-'+str(i)
                a.ObjectColor=System.Drawing.Color.FromArgb(20+i,40,60)
                a.ColorSource=Rhino.DocObjects.ObjectColorSource.ColorFromObject
                a.SetUserString('Code','attribute-'+str(i));g.SetUserString('Code','geometry-'+str(i))
                ids.append(doc.Objects.AddBrep(g,a))
                if ids[-1]==System.Guid.Empty:raise ValueError('Boolean2Objects source insertion failed')
            finally:g.Dispose();a.Dispose()
        for members in [[key] for key in ids]+[ids]:
            groups.append(doc.Groups.Add('Viboceros Boolean2Objects '+str(System.Guid.NewGuid()),members))
        doc.ClearUndoRecords(True)
        view=doc.Views.ActiveView;viewport=view.ActiveViewport
        original=Rhino.DocObjects.ViewportInfo(viewport);target=viewport.CameraTarget;name=viewport.Name
        viewport.SetProjection(Rhino.Display.DefinedViewportProjection.Top,'Boolean2Objects owned',False)
        viewport.ZoomBoundingBox(G.BoundingBox(G.Point3d(-5.,-5.,-1.),G.Point3d(5.,5.,4.)))
        pixel=viewport.WorldToClient(G.Point3d(-4.,-4.,0.))
        screen=view.ClientToScreen(System.Drawing.Point(int(pixel.X),int(pixel.Y)))
        before=snapshot()
        marker='@boolean-two:'+op['id']
        with open(os.path.join(root,'worker-progress.log'),'a') as f:
            f.write('PICK '+marker+' '+str(screen.X)+' '+str(screen.Y)+'\n');f.flush()
        select=' '.join('_SelID '+str(key) for key in ids)
        macro='_Boolean2Objects _DeleteInput=_'+('Yes' if spec.get('delete',True) else 'No')+' '+select+' _Pause'
        try:command=invoke(macro,'Boolean2Objects')
        finally:
            host['_record_progress']('PICK_DONE '+marker)
            viewport.SetViewProjection(original,False);viewport.SetCameraTarget(target,False);viewport.Name=name
        result=dict(case=op['case'],bounds=spec['bounds'],cycles=spec['cycles'],delete=spec.get('delete',True),cancel=spec.get('cancel',False),before=before,command=command)
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

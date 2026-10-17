"""Flow Line getters, temporary-guide lifetime and complete owner/history states."""
import math
if globals().get('__package__'):
    from .flow_probe import validate as validate_flow
    from .geometry_validity_probe import validate as validate_source
else:
    from flow_probe import validate as validate_flow
    from geometry_validity_probe import validate as validate_source


def validate(op,allow_selected_guides=False):
    allowed={'op','id','base','target','source','options','group','base_line','target_line','selected','edit','local_radii'}
    if not isinstance(op,dict)or op.get('op')!='flow_line'or set(op)-allowed or not {'id','base','target','source'}<=set(op):raise ValueError('invalid Flow Line recipe')
    for k in ('group','base_line','target_line','edit'):
        if type(op.get(k,k=='base_line'))is not bool:raise ValueError('invalid Flow Line toggle')
    if 'local_radii'in op:
        radii=op['local_radii']
        if not isinstance(radii,list)or len(radii)!=2 or any(type(r)not in(int,float)or not 0<=r<=1e6 for r in radii)or radii[0]==radii[1]:raise ValueError('invalid Flow local radii')
    if not allow_selected_guides and 'local_radii'not in op and not op.get('base_line',True)and not op.get('target_line',False):raise ValueError('Flow Line needs at least one drawn guide')
    for role in ('base','target'):
        if op.get(role+'_line',role=='base'):
            if op[role].get('type')!='line':raise ValueError('drawn guide needs two endpoints')
            points=[op[role].get(k)for k in ('start','end')]
            if any(not isinstance(p,list)or len(p)!=3 or any(type(v)not in(int,float)or math.isnan(v)or math.isinf(v)for v in p)for p in points):raise ValueError('invalid drawn guide endpoints')
            delta=[float(b)-float(a)for a,b in zip(*points)]
            if any(math.isnan(v)or math.isinf(v)for v in delta)or not any(delta):raise ValueError('drawn guide must have finite nonzero displacement')
    validate_flow(dict(op='flow',id=op['id'],base=op['base'],target=op['target'],points=[[0,0,0]]*4,options=op.get('options',{})))
    validate_source(dict(op='geometry_validity',id=op['id'],source=op['source']))
    if op['source'].get('type')not in('point_cloud','nurbs','surface','mesh'):raise ValueError('unsupported Flow Line witness family')
    selected=op.get('selected',[])
    if not isinstance(selected,list)or len(selected)>64 or any(type(i)is not int or not 0<=i<64 for i in selected)or len(set(selected))!=len(selected):raise ValueError('invalid Flow Line grips')
    if selected and op['source']['type']=='point_cloud':raise ValueError('point clouds have no owner grips')
    if op.get('edit')and(not selected or op['source']['type']!='nurbs'):raise ValueError('edit grips need a selected curve basis')
    if op.get('options',{}).get('roadlike')or op.get('options',{}).get('stairlike'):raise ValueError('advanced Flow Line modes are not qualified')


def run(op,host,iterations,tolerance,roadlike_axes=None):
    validate(op,allow_selected_guides=roadlike_axes is not None)
    if iterations!=1:raise ValueError('Flow Line capture requires one iteration')
    from join_probe import observe_command
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;o=op.get('options',{})
    before=set(obj.Id for obj in doc.Objects);groups=set(g.Index for g in doc.Groups if not g.IsDeleted)
    curves=[host['_object_source'](op[k],tolerance)for k in('base','target')];geometry=host['_object_source'](op['source'],tolerance)
    try:
        for role,curve in zip(('base','target'),curves):
            if op.get(role+'_line',role=='base')and curve.GetLength()<=doc.ModelAbsoluteTolerance:raise ValueError('drawn guide is too short for capture tolerance')
        guides=[]
        for role,curve in zip(('base','target'),curves):
            if o.get('reverse_'+role):curve.Reverse()
            guides.append(None if op.get(role+'_line',role=='base')else doc.Objects.AddCurve(curve))
        source=doc.Objects.Add(geometry)
        if op.get('group'):doc.Groups.Add('Owned Flow Line',[source])
        definition=lambda key:host['_nurbs_curve_definition'](doc.Objects.FindId(key).Geometry.ToNurbsCurve())
        originals={key:definition(key)for key in guides if key is not None};doc.Objects.UnselectAll()
        if op.get('selected'):
            obj=doc.Objects.FindId(source)
            if op.get('edit'):
                doc.Objects.Select(source,True);R.RhinoApp.RunScript('_EditPtOn',True);doc.Objects.UnselectAll();obj=doc.Objects.FindId(source)
            else:obj.GripsOn=True
            grips=obj.GetGrips()or[]
            if any(i>=len(grips)for i in op['selected']):raise ValueError('Flow Line grip index missing')
            for i in op['selected']:grips[i].Select(True)
        else:doc.Objects.Select(source,True)
        def snapshot():
            outputs=[];objects=[obj for obj in doc.Objects if obj.Id not in before];active=sorted(g.Index for g in doc.Groups if not g.IsDeleted and g.Index not in groups)
            for obj in objects:
                if obj.Id in originals:continue
                g=obj.Geometry;row=dict(original=obj.Id==source,peer=False,selected=bool(obj.IsSelected(False)),group_count=obj.GroupCount,groups=sorted(active.index(i)for i in(obj.GetGroupList()or[])if i in active),grips_on=bool(obj.GripsOn),grips=[dict(index=int(p.Index),point=host['_xyz'](p.CurrentLocation),selected=bool(p.IsSelected(False)))for p in(obj.GetGrips()or[])])
                if isinstance(g,G.PointCloud):row['points']=[host['_xyz'](p.Location)for p in g]
                elif isinstance(g,G.Curve):row['curve']=host['_nurbs_curve_definition'](g.ToNurbsCurve())
                elif isinstance(g,G.Brep):row['surface']=host['_nurbs_surface_definition'](g.Faces[0].UnderlyingSurface().ToNurbsSurface())
                elif isinstance(g,G.Mesh):row['mesh']=host['_polygon_mesh_value'](g)
                else:raise ValueError('unexpected Flow Line output')
                outputs.append(row)
            return dict(outputs=outputs,object_count=len(objects),guide_unchanged=all(definition(k)==v for k,v in originals.items()),guide_selected=any(bool(doc.Objects.FindId(k).IsSelected(False))for k in originals),group_count=len(active))
        doc.ClearUndoRecords(True);seed=snapshot()
        point=lambda p:'w'+','.join(repr(float(v))for v in p)
        flags=' '.join('_'+key+'=_'+('Yes'if o.get(field,default)else'No')for key,field,default in [('Copy','copy',True),('Rigid','rigid',False),('Stretch','stretch',False),('PreserveStructure','preserve_structure',False)])
        if o.get('rigid'):flags+=' _RigidGroups=_'+('Yes'if o.get('rigid_groups',True)else'No')
        flags+=' _Roadlike=_'+('Yes'if roadlike_axes is not None else 'No')+' _Local=_'+('Yes'if 'local_radii'in op else'No')
        picks=[]
        for role,key in zip(('base','target'),guides):
            if key is None:
                a,b=op[role]['start'],op[role]['end']
                if o.get('reverse_'+role):a,b=b,a
                picks.append(' _Line '+point(a)+' '+point(b))
            else:picks.append(" '_-SelID "+str(key))
        axes=[]
        if roadlike_axes is not None:
            for curve,up in zip(curves,roadlike_axes):
                if up is None:axes.append(' '+point(host['_xyz'](curve.PointAtStart))+' _Automatic')
                else:
                    start=host['_xyz'](curve.PointAtStart)
                    axes.append(' '+point(start)+' '+point([a+b for a,b in zip(start,up)]))
        radii=' '+ ' '.join(repr(float(r))for r in op['local_radii'])if 'local_radii'in op else''
        macro='_Flow '+flags+''.join(picks)+''.join(axes)+radii
        marker='Owned Flow Line '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        try:success,after,events=observe_command(R.Commands.Command,'Flow',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        except ValueError as error:
            if 'local_radii'in op:raise ValueError(str(error)+'; '+R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1][-2000:])
            raise
        result=dict(before=seed,after=after,after_script=snapshot(),events=events,command_returned=bool(success),macro=macro,history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip())
        result['undo_command_returned']=bool(R.RhinoApp.RunScript('_Undo',False));result['undo']=snapshot()
        result['redo_command_returned']=bool(R.RhinoApp.RunScript('_Redo',False));result['redo']=snapshot()
        return result,0
    finally:
        for obj in list(doc.Objects):
            if obj.Id not in before:
                if obj.GripsOn:obj.GripsOn=False
                doc.Objects.Delete(obj.Id,True)
        for g in list(doc.Groups):
            if not g.IsDeleted and g.Index not in groups:doc.Groups.Delete(g.Index)
        for curve in curves:curve.Dispose()
        geometry.Dispose()

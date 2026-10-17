"""Authored Flow control/edit grip inputs and complete owner observations."""
import re
if globals().get('__package__'):
    from .geometry_validity_probe import validate as validate_geometry
    from .flow_probe import validate as validate_flow
else:
    from geometry_validity_probe import validate as validate_geometry
    from flow_probe import validate as validate_flow


def validate(op):
    allowed={'op','id','base','target','source','selected','options','edit','parent','peer','group','postselected'}
    if not isinstance(op,dict)or set(op)-allowed or not {'op','id','base','target','source','selected'}<=set(op)or op['op']!='flow_grips':raise ValueError('invalid Flow grip recipe')
    validate_flow(dict(op='flow',id=op['id'],base=op['base'],target=op['target'],points=[[0,0,0]]*4,options=op.get('options',{})))
    validate_geometry(dict(op='geometry_validity',id=op['id'],source=op['source']))
    if op['source'].get('type')not in('nurbs','surface','mesh'):raise ValueError('unsupported Flow grip owner')
    if not isinstance(op['selected'],list)or not 1<=len(op['selected'])<=64 or any(type(i)is not int or not 0<=i<64 for i in op['selected'])or len(set(op['selected']))!=len(op['selected']):raise ValueError('invalid Flow grip indices')
    for key in('edit','parent','peer','group','postselected'):
        if type(op.get(key,False))is not bool:raise ValueError('invalid Flow grip toggle')
    if op.get('edit')and op['source']['type']!='nurbs':raise ValueError('edit points require a curve')
    if op.get('postselected')and(op.get('parent')or op.get('peer')):raise ValueError('postselection captures grip-only SelPt')
    if op.get('options',{}).get('roadlike')or op.get('options',{}).get('stairlike'):raise ValueError('advanced Flow grip modes are not qualified')


def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('Flow grips require one iteration')
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    before=set(o.Id for o in doc.Objects);groups=set(g.Index for g in doc.Groups if not g.IsDeleted)
    base=host['_object_source'](op['base'],tolerance);target=host['_object_source'](op['target'],tolerance);geometry=host['_object_source'](op['source'],tolerance)
    try:
        options=op.get('options',{})
        if options.get('reverse_base'):base.Reverse()
        if options.get('reverse_target'):target.Reverse()
        a=doc.Objects.AddCurve(base);b=doc.Objects.AddCurve(target);source=doc.Objects.Add(geometry);peer=None
        if op.get('peer'):peer=doc.Objects.AddPoint(G.Point3d(6.,1.,0.))
        if op.get('group'):doc.Groups.Add('Owned Flow grips',[source]+([peer]if peer else[]))
        definition=lambda key:host['_nurbs_curve_definition'](doc.Objects.FindId(key).Geometry.ToNurbsCurve())
        guides=[definition(a),definition(b)];doc.Objects.UnselectAll();obj=doc.Objects.FindId(source)
        if op.get('edit'):
            doc.Objects.Select(source,True);R.RhinoApp.RunScript('_EditPtOn',True);doc.Objects.UnselectAll();obj=doc.Objects.FindId(source)
        else:obj.GripsOn=True
        grips=obj.GetGrips()or[]
        if not grips or any(i>=len(grips)for i in op['selected']):raise ValueError('native Flow grip index missing')
        if op.get('postselected')and set(op['selected'])!=set(range(len(grips))):raise ValueError('postselection must capture every grip')
        if not op.get('postselected'):
            for i in op['selected']:grips[i].Select(True)
            if op.get('parent'):doc.Objects.Select(source,True)
            if peer:doc.Objects.Select(peer,True)
        def snapshot():
            outputs=[];active=sorted(g.Index for g in doc.Groups if not g.IsDeleted and g.Index not in groups)
            for o in doc.Objects:
                if o.Id in before or o.Id in(a,b):continue
                g=o.Geometry;row=dict(original=o.Id==source,peer=o.Id==peer,selected=bool(o.IsSelected(False)),group_count=o.GroupCount,groups=sorted(active.index(i)for i in(o.GetGroupList()or[])if i in active),grips_on=bool(o.GripsOn),grips=[dict(index=int(p.Index),point=host['_xyz'](p.CurrentLocation),selected=bool(p.IsSelected(False)))for p in(o.GetGrips()or[])])
                if isinstance(g,G.Curve):row['curve']=host['_nurbs_curve_definition'](g.ToNurbsCurve())
                elif isinstance(g,G.Brep):row['surface']=host['_nurbs_surface_definition'](g.Faces[0].UnderlyingSurface().ToNurbsSurface())
                elif isinstance(g,G.Mesh):row['mesh']=host['_polygon_mesh_value'](g)
                elif isinstance(g,G.Point):row['point']=host['_xyz'](g.Location)
                else:raise ValueError('unexpected Flow grip output')
                outputs.append(row)
            return dict(outputs=outputs,group_count=len(active),guides_selected=[bool(doc.Objects.FindId(k).IsSelected(False))for k in(a,b)],guides_unchanged=guides==[definition(a),definition(b)])
        doc.ClearUndoRecords(True)
        seed=snapshot();flags=' '.join('_'+key+'=_'+('Yes'if options.get(field,default)else'No')for key,field,default in [('Copy','copy',True),('Rigid','rigid',False),('Stretch','stretch',False),('PreserveStructure','preserve_structure',False)])
        if options.get('rigid'):flags+=' _RigidGroups=_'+('Yes'if options.get('rigid_groups',True)else'No')
        flags+=' _Roadlike=_No _Local=_No'
        marker='Owned Flow grips '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_Flow '+('_SelPt _Enter 'if op.get('postselected')else'')+flags+" '_-SelID "+str(a)+" '_-SelID "+str(b)
        from join_probe import observe_command
        returned,after,events=observe_command(R.Commands.Command,'Flow',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        result=dict(before=seed,after=after,after_script=snapshot(),events=events,command_returned=bool(returned),macro=macro,history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1].strip())
        result['undo_command_returned']=bool(R.RhinoApp.RunScript('_Undo',False));result['undo']=snapshot()
        result['redo_command_returned']=bool(R.RhinoApp.RunScript('_Redo',False));result['redo']=snapshot()
        return result,0
    finally:
        for o in list(doc.Objects):
            if o.Id not in before:
                if o.GripsOn:o.GripsOn=False
                doc.Objects.Delete(o.Id,True)
        for g in list(doc.Groups):
            if not g.IsDeleted and g.Index not in groups:doc.Groups.Delete(g.Index)
        base.Dispose();target.Dispose();geometry.Dispose()

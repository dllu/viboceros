"""Bounded public OffsetCrvOnSrf getters on an empty owned document."""
import re
CASES={'distance':[], 'flip_number':['Number=3','Flip'], 'negative':[], 'nurbs_line':['Number=1'], 'bezier':[], 'trimmed_line':[], 'postselect':['Number=1'], 'trimmed_number':['Number=3']}
CURVED_CASES={'curved_axis':1.,'curved_axis_negative':-1.,'curved_ring':1.,'curved_ring_negative':-1.,'curved_diagonal':1.,'curved_warped':.25,'curved_warped_negative':-.25,'curved_warped_diagonal':.25}
GENERAL_CASES={'general_diagonal':.05,'general_reversed':.05,'general_iso':-.05,'general_rational':.05}
CORNER_CASES={'corner_open_inner':.5,'corner_open_outer':-.5,'corner_closed_inner':.5,'corner_closed_outer':-.5,'corner_zigzag_inner':.5,'corner_zigzag_outer':-.5,'corner_reverse':.5,'corner_tilted':-.5,'corner_polycurve':-.5}
THROUGH_CORNER_CASES={
    'through_corner_inner':('corner_open_inner',[[-.5,-1.5,0.]]),
    'through_corner_outer':('corner_open_outer',[[.375,-2.375,0.]]),
    'through_corner_outer_end':('corner_open_outer',[[.5,-2.,0.]]),
    'through_corner_closed_inner':('corner_closed_inner',[[1.5,-1.5,0.]]),
    'through_corner_closed_outer':('corner_closed_outer',[[2.375,-2.375,0.]]),
    'through_corner_repeat':('corner_open_outer',[[.375,-2.375,0.],[-.5,-1.5,0.]]),
    'through_corner_reverse':('corner_reverse',[[.375,-2.375,0.]]),
    'through_corner_off_surface':('corner_open_outer',[[.375,-2.375,3.]]),
    'through_corner_tilted':('corner_tilted',[[0.375, -1.960172085410486, -1.341025874313209]]),
    'through_corner_on_source':('corner_open_inner',[[0.,-2.,0.]]),
    'through_corner_polycurve':('corner_polycurve',[[.375,-2.375,0.]]),
    'through_corner_cancel':('corner_open_outer',[[.375,-2.375,0.]]),
}
SELECT_CORNER_CASES={
    'through_corner_select_inner':('corner_open_inner',[[-.5,-1.5,0.],[-1.,-1.,0.]]),
    'through_corner_select_limits':('corner_open_outer',[[.5,-2.,0.],[0.,-2.5,0.]]),
    'through_corner_select_on_source':('corner_open_inner',[[0.,-2.,0.],[-.5,-1.5,0.]]),
    'through_corner_select_unresolved':('corner_open_outer',[[.375,-2.375,0.],[-.5,-1.5,0.]]),
}
def corner_source(G,case):
    if case=='corner_polycurve':
        source=G.PolyCurve()
        a=G.LineCurve(G.Point3d(-4,-2,0),G.Point3d(0,-2,0))
        b=G.Curve.CreateControlPointCurve([G.Point3d(*p)for p in[[0,-2,0],[0,-1,0],[2,2,0],[3,3,0]]],3)
        try:
            if not source.Append(a)or not source.Append(b):raise ValueError('owned polycurve construction failed')
        finally:a.Dispose();b.Dispose()
    else:
        points=[[-2,-2,0],[2,-2,0],[2,2,0],[-2,2,0],[-2,-2,0]]if 'closed'in case else[[-4,-2,0],[0,-2,0],[0,1,0],[4,1,0]]if 'zigzag'in case else[[-4,-2,0],[0,-2,0],[0,4,0]]
        source=G.PolylineCurve([G.Point3d(*p)for p in points])
        if case=='corner_reverse':source.Reverse()
    return source
def general_source(host,case):
    G=host['Rhino'].Geometry
    definition=dict(type='surface',degree_u=2,degree_v=2,control_point_count_u=3,control_point_count_v=3,
        knots_u=[0.,0.,0.,1.,1.,1.],knots_v=[0.,0.,0.,1.,1.,1.],
        control_points=[dict(point=[float(u),float(v),.2*u*v*v],weight=1.1 if case=='general_rational'and u==v==1 else 1.)for v in range(3)for u in range(3)])
    basis=host['_object_source'](definition,1e-9)
    try:
        if case=='general_iso':
            iso=basis.IsoCurve(0,.5)
            try:source=iso.Trim(G.Interval(.3,.7))
            finally:iso.Dispose()
        else:
            uv=G.LineCurve(G.Point3d(.3,.3,0),G.Point3d(.7,.7,0))
            try:source=basis.Pushup(uv,1e-9)
            finally:uv.Dispose()
        if source is None or not source.IsValid:raise ValueError('invalid owned general surface curve')
        if case=='general_reversed'and not source.Reverse():
            source.Dispose();raise ValueError('owned source reversal failed')
        return basis,source
    except BaseException:
        basis.Dispose();raise
def curved_source(G,case):
    warped=case.startswith('curved_warped')
    basis=G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p)for p in[[-5,-5,0],[5,-5,0],[5,5,3],[-5,5,-1]]])if warped else G.Cylinder(G.Circle(G.Plane.WorldXY,3.),6.).ToNurbsSurface()
    try:
        if 'diagonal'in case:
            uv=G.LineCurve(G.Point3d(.25,.3,0),G.Point3d(.7,.8,0))if warped else G.LineCurve(G.Point3d(.2,1.5,0),G.Point3d(1.8,4.5,0))
            try:source=basis.Pushup(uv,1e-9)
            finally:uv.Dispose()
        else:
            iso=basis.IsoCurve(0,.5 if warped else 3.)if warped or 'ring'in case else basis.IsoCurve(1,.6)
            try:
                a,b=(.25,.75)if warped else(.3,1.3)if 'ring'in case else(1.5,4.5)
                source=iso.Trim(G.Interval(a,b))
            finally:iso.Dispose()
        if source is None or not source.IsValid:raise ValueError('invalid owned shortened surface curve')
        return basis,source
    except BaseException:
        basis.Dispose();raise

THROUGH_CASES={
    'through_repeat':[[0.,2.,0.],[1.,-1.,0.]],
    'through_number':[[0.,2.,0.]],
    'through_off_surface':[[0.,2.,5.]],
    'through_on_source':[[0.,0.,0.]],
    'through_cancel':[[0.,2.,0.]],
    'through_trimmed':[[0.,2.,0.],[0.,1.,0.]],
    'through_cylinder':[[3.,0.,3.],[0.,3.,3.]],
    'through_sphere':[[4.,0.,0.]],
    'through_bezier':[[0.,2.,0.]],
    'through_extension':[[8.,2.,0.]],
    'through_trimmed_tie_first':[[0.,1.,0.]],
    'through_trimmed_tie_below':[[0.,-2.,0.],[0.,1.,0.]],
    'through_trimmed_rotated':[[ -2.,0.,0.],[-1.,0.,0.]],
    'through_trimmed_reverse_u':[[0.,1.,0.]],
    'through_trimmed_reverse_v':[[0.,1.,0.]],
    'through_trimmed_split_bottom':[[0.,1.,0.]],
}
SELECT_CASES={
    'through_select':[[0.,2.,5.]],
    'through_select_multiple':[[0.,2.,0.],[1.,-1.,0.]],
    'through_select_reverse':[[0.,2.,0.],[1.,-1.,0.]],
    'through_select_on_source':[[0.,0.,0.],[0.,2.,0.]],
    'through_select_cancel':[[0.,2.,0.]],
    'through_select_empty':[],
    'through_select_all_on_source':[[0.,0.,0.],[8.,0.,5.]],
    'through_select_same_offset':[[0.,2.,0.],[1.,2.,0.]],
}
def recipe(case):
    if case in SELECT_CORNER_CASES:
        indices=list(range(2,2+len(SELECT_CORNER_CASES[case][1])))
        return [dict(curve=1),'ThroughPoint','Select']+[dict(curve=i)for i in indices]+['Enter'],[['Select base surface'],['Offset distance'],['Through point']]+[['Select points']for _ in indices+[0]]
    if case in THROUGH_CORNER_CASES:
        points=THROUGH_CORNER_CASES[case][1]
        actions=[dict(curve=1),'Number=3','Flip','ThroughPoint']+points+['Cancel'if case=='through_corner_cancel'else'Enter']
        return actions,[['Select base surface']]+[['Offset distance']for _ in range(3)]+[['Point','Select point','Through point','Offset point']for _ in points+['Enter']]
    if case in CORNER_CASES:
        return [dict(curve=1),'Number=1',CORNER_CASES[case]],[['Select base surface'],['Offset distance'],['Offset distance']]
    if case in GENERAL_CASES:
        return [dict(curve=1),'Number=1',GENERAL_CASES[case]],[['Select base surface'],['Offset distance'],['Offset distance']]
    if case in CURVED_CASES:
        return [dict(curve=1),'Number=1',CURVED_CASES[case]],[['Select base surface'],['Offset distance'],['Offset distance']]
    if case in SELECT_CASES:
        indices=list(range(2,2+len(SELECT_CASES[case])))
        if case=='through_select_reverse':indices.reverse()
        return [dict(curve=1),'ThroughPoint','Select']+[dict(curve=i)for i in indices]+['Cancel'if case=='through_select_cancel'else'Enter'],[['Select base surface'],['Offset distance'],['Through point']]+[['Select points']for _ in indices+[0]]
    if case in THROUGH_CASES:
        options=['Number=3']if case=='through_number'else[]
        actions=[dict(curve=1)]+options+['ThroughPoint']+THROUGH_CASES[case]+['Cancel'if case=='through_cancel'else'Enter']
        return actions,[['Select base surface']]+[['Offset distance']for _ in options+['ThroughPoint']]+[['Point','Select point','Through point','Offset point']for _ in THROUGH_CASES[case]+['Enter']]

    actions=([dict(curve=0)]if case=='postselect'else[])+[dict(curve=1)]+CASES[case]+[-1. if case=='negative'else 1.]
    gates=([['Select curve']]if case=='postselect'else[])+[['Select base surface']]+[['Offset distance']for _ in CASES[case]+[1]]
    return actions,gates
def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','case','actions','action_prompts'}or op['op']not in('surface_offset_command','surface_offset_through_point'):raise ValueError('invalid surface offset command recipe')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id'])is None or not isinstance(op['case'],str)or op['case']not in(dict(dict(dict(CASES,**CURVED_CASES),**GENERAL_CASES),**CORNER_CASES)if op['op']=='surface_offset_command'else dict(dict(dict(THROUGH_CASES,**SELECT_CASES),**THROUGH_CORNER_CASES),**SELECT_CORNER_CASES)):raise ValueError('invalid owned surface offset command case')
    actions,gates=recipe(op['case'])
    import json
    if json.dumps(op['actions'],sort_keys=True)!=json.dumps(actions,sort_keys=True)or op['action_prompts']!=gates:raise ValueError('only fixed surface offset getters are allowed')
def request():
    operations=[]
    for case in CASES:
        actions,gates=recipe(case)
        operations.append(dict(op='surface_offset_command',id=case,case=case,actions=actions,action_prompts=gates))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=operations)
def through_request():
    q=request();q['operations']=[]
    for case in THROUGH_CASES:
        actions,gates=recipe(case)
        q['operations'].append(dict(op='surface_offset_through_point',id=case,case=case,actions=actions,action_prompts=gates))
    return q
def curved_request():
    q=request();q['operations']=[]
    for case in CURVED_CASES:
        actions,gates=recipe(case)
        q['operations'].append(dict(op='surface_offset_command',id=case,case=case,actions=actions,action_prompts=gates))
    return q
def general_request():
    q=request();q['operations']=[]
    for case in GENERAL_CASES:
        actions,gates=recipe(case)
        q['operations'].append(dict(op='surface_offset_command',id=case,case=case,actions=actions,action_prompts=gates))
    return q
def corner_request():
    q=request();q['operations']=[]
    for case in CORNER_CASES:
        actions,gates=recipe(case)
        q['operations'].append(dict(op='surface_offset_command',id=case,case=case,actions=actions,action_prompts=gates))
    return q
def corner_through_request():
    q=request();q['operations']=[]
    for case in THROUGH_CORNER_CASES:
        actions,gates=recipe(case)
        q['operations'].append(dict(op='surface_offset_through_point',id=case,case=case,actions=actions,action_prompts=gates))
    return q
def corner_select_request():
    q=request();q['operations']=[]
    for case in SELECT_CORNER_CASES:
        actions,gates=recipe(case)
        q['operations'].append(dict(op='surface_offset_through_point',id=case,case=case,actions=actions,action_prompts=gates))
    return q
def select_request():
    q=request();q['operations']=[]
    for case in SELECT_CASES:
        actions,gates=recipe(case)
        q['operations'].append(dict(op='surface_offset_through_point',id=case,case=case,actions=actions,action_prompts=gates))
    return q
def validate_request(q):
    if q.get('iterations')!=1 or type(q.get('iterations'))is not int or not isinstance(q.get('operations'),list)or not 1<=len(q['operations'])<=16:raise ValueError('bounded surface offset command capture required')
    for op in q['operations']:validate(op)
    if len(set(op['id']for op in q['operations']))!=len(q['operations']):raise ValueError('unique case names required')
def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    from conic_getter_input import drive
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('an empty owned idle document is required')
    source=G.LineCurve(G.Point3d(-4,0,0),G.Point3d(4,0,0));surface=G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p)for p in[[-10,-10,0],[10,-10,0],[10,10,0],[-10,10,0]]]).ToBrep()
    corner_case=SELECT_CORNER_CASES[op['case']][0]if op['case']in SELECT_CORNER_CASES else THROUGH_CORNER_CASES[op['case']][0]if op['case']in THROUGH_CORNER_CASES else op['case']
    if corner_case in CORNER_CASES:
        source.Dispose();source=corner_source(G,corner_case)
        if corner_case=='corner_tilted':
            rotation=G.Transform.Rotation(.6,G.Vector3d.XAxis,G.Point3d.Origin);source.Transform(rotation);surface.Transform(rotation)
    if op['case']=='nurbs_line':
        converted=source.ToNurbsCurve();source.Dispose();source=converted
    if op['case']in('bezier','through_bezier'):
        source.Dispose();source=G.Curve.CreateControlPointCurve([G.Point3d(*p)for p in[[-4,0,0],[-2,3,0],[2,-3,0],[4,0,0]]],3)
    if op['case']in('trimmed_line','trimmed_number')or op['case'].startswith('through_trimmed'):
        surface.Dispose()
        polygons=[G.PolylineCurve([G.Point3d(*p)for p in points])for points in[[[-5,-5,0],[5,-5,0],[5,5,0],[-5,5,0],[-5,-5,0]],[[-1,.5,0],[1,.5,0],[1,1.5,0],[-1,1.5,0],[-1,.5,0]]]]
        try:
            pieces=list(G.Brep.CreatePlanarBreps(polygons,1e-6)or[])
            if len(pieces)!=1:raise ValueError('one owned planar trimmed face required')
            surface=pieces[0]
        finally:
            for p in polygons:p.Dispose()
    if op['case']=='through_trimmed_split_bottom':
        e=surface.Edges[4];count=surface.Edges.SplitEdgeAtParameters(4,[e.Domain.Mid]);surface.Compact();surface.SetTolerancesBoxesAndFlags()
        if count<1 or not surface.IsValid:raise ValueError('owned bottom edge split failed: count=%s valid=%s'%(count,surface.IsValid))
    if op['case']=='through_trimmed_rotated':
        rotation=G.Transform.Rotation(1.5707963267948966,G.Vector3d.ZAxis,G.Point3d.Origin);source.Transform(rotation);surface.Transform(rotation)
    if op['case']in('through_trimmed_reverse_u','through_trimmed_reverse_v'):
        if surface.Faces[0].Reverse(0 if op['case'].endswith('_u')else 1,True)is None:raise ValueError('owned face reversal failed')
    if op['case']in('through_cylinder','through_sphere'):
        source.Dispose();surface.Dispose()
        basis=G.Cylinder(G.Circle(G.Plane.WorldXY,3.),6.).ToNurbsSurface()if op['case']=='through_cylinder'else G.Sphere(G.Point3d.Origin,4.).ToNurbsSurface()
        try:
            source=basis.IsoCurve(1,.6)if op['case']=='through_cylinder'else basis.IsoCurve(0,.2)
            surface=basis.ToBrep()
        finally:basis.Dispose()
    if op['case']in GENERAL_CASES:
        source.Dispose();surface.Dispose()
        basis,source=general_source(host,op['case'])
        try:surface=basis.ToBrep()
        finally:basis.Dispose()
    elif op['case']in CURVED_CASES:
        source.Dispose();surface.Dispose()
        basis,source=curved_source(G,op['case'])
        try:surface=basis.ToBrep()
        finally:basis.Dispose()
    ids=[];layers=[];old_layer=doc.Layers.CurrentLayerIndex
    def snapshot():
        rows=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=o.Geometry;role='source'if o.Id==ids[0]else'target'if o.Id==ids[1]else'reference'if o.Id in ids[2:]else'output'
            row=dict(role=role,selected=bool(o.IsSelected(False)),layer=doc.Layers[o.Attributes.LayerIndex].Name,name=o.Attributes.Name,crc=int(g.DataCRC(0)))
            if isinstance(g,G.Curve):row['curve']=host['_nurbs_curve_definition'](g)
            if isinstance(g,G.Point):row['point']=host['_xyz'](g.Location)
            rows.append(row)
        return rows
    def definition(curve):
        n=curve.ToNurbsCurve()
        try:return host['_nurbs_curve_definition'](n)
        finally:n.Dispose()
    target_definition=None;constraints=[]
    if op['case'].startswith('through_trimmed'):
        target_definition=dict(vertices=[host['_xyz'](v.Location)for v in surface.Vertices],edges=[dict(vertices=[e.StartVertex.VertexIndex,e.EndVertex.VertexIndex],curve=definition(e))for e in surface.Edges],reversed=bool(surface.Faces[0].OrientationIsReversed),loops=[dict(type=str(l.LoopType),trims=[dict(edge=t.Edge.EdgeIndex,reversed=bool(t.IsReversed()),curve=definition(t))for t in l.Trims])for l in surface.Faces[0].Loops])
        constraints=[dict(pick=a,sdk_closest=host['_xyz'](surface.ClosestPoint(G.Point3d(*a))))for a in op['actions']if isinstance(a,list)]
    try:
        record=doc.BeginUndoRecord('Owned surface offset scene')
        try:
            for name in ['Surface offset source','Surface offset output']:
                layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer))
            a=R.DocObjects.ObjectAttributes();a.Name='Source';a.LayerIndex=layers[0];ids.append(doc.Objects.AddCurve(source,a))
            a=R.DocObjects.ObjectAttributes();a.Name='Target';ids.append(doc.Objects.AddBrep(surface,a))
            for p in SELECT_CASES.get(op['case'],SELECT_CORNER_CASES.get(op['case'],(None,[]))[1]):
                a=R.DocObjects.ObjectAttributes();a.Name='Reference';ids.append(doc.Objects.AddPoint(G.Point3d(*p),a))
            doc.Layers.SetCurrentLayerIndex(layers[1],True)
            if op['case']!='postselect':doc.Objects.Select(ids[0])
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);before=snapshot();traces=[];marker='Owned surface offset '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        def execute():
            result,trace=drive(op,host,ids,'OffsetCrvOnSrf');traces.append(trace);return result
        success,after,events=observe_command(R.Commands.Command,'OffsetCrvOnSrf',execute,snapshot,lambda:[],True)
        after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(target_definition=target_definition,constraints=constraints,surface_definition=host['_nurbs_surface_definition'](surface.Surfaces[0]),success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,input_trace=traces[0],events=events,history=history),0
    finally:
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
        source.Dispose();surface.Dispose()

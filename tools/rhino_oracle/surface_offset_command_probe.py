"""Bounded public OffsetCrvOnSrf getters on an empty owned document."""
import re
CASES={'distance':[], 'flip_number':['Number=3','Flip'], 'negative':[], 'nurbs_line':['Number=1'], 'bezier':[], 'trimmed_line':[], 'postselect':['Number=1'], 'trimmed_number':['Number=3']}
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
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id'])is None or not isinstance(op['case'],str)or op['case']not in(CASES if op['op']=='surface_offset_command'else dict(THROUGH_CASES,**SELECT_CASES)):raise ValueError('invalid owned surface offset command case')
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
            for p in SELECT_CASES.get(op['case'],[]):
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

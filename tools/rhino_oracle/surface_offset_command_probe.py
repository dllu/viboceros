"""Bounded public OffsetCrvOnSrf getters on an empty owned document."""
import re
CASES={'distance':[], 'flip_number':['Number=3','Flip'], 'negative':[], 'nurbs_line':['Number=1'], 'bezier':[], 'trimmed_line':[], 'postselect':['Number=1'], 'trimmed_number':['Number=3']}
def recipe(case):
    actions=([dict(curve=0)]if case=='postselect'else[])+[dict(curve=1)]+CASES[case]+[-1. if case=='negative'else 1.]
    gates=([['Select curve']]if case=='postselect'else[])+[['Select base surface']]+[['Offset distance']for _ in CASES[case]+[1]]
    return actions,gates
def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','case','actions','action_prompts'}or op['op']!='surface_offset_command':raise ValueError('invalid surface offset command recipe')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id'])is None or not isinstance(op['case'],str)or op['case']not in CASES:raise ValueError('invalid owned surface offset command case')
    actions,gates=recipe(op['case'])
    import json
    if json.dumps(op['actions'],sort_keys=True)!=json.dumps(actions,sort_keys=True)or op['action_prompts']!=gates:raise ValueError('only fixed surface offset getters are allowed')
def request():
    operations=[]
    for case in CASES:
        actions,gates=recipe(case)
        operations.append(dict(op='surface_offset_command',id=case,case=case,actions=actions,action_prompts=gates))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=operations)
def validate_request(q):
    if q.get('iterations')!=1 or type(q.get('iterations'))is not int or not isinstance(q.get('operations'),list)or not 1<=len(q['operations'])<=8:raise ValueError('bounded surface offset command capture required')
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
    if op['case']=='bezier':
        source.Dispose();source=G.Curve.CreateControlPointCurve([G.Point3d(*p)for p in[[-4,0,0],[-2,3,0],[2,-3,0],[4,0,0]]],3)
    if op['case']in('trimmed_line','trimmed_number'):
        surface.Dispose()
        polygons=[G.PolylineCurve([G.Point3d(*p)for p in points])for points in[[[-5,-5,0],[5,-5,0],[5,5,0],[-5,5,0],[-5,-5,0]],[[-1,.5,0],[1,.5,0],[1,1.5,0],[-1,1.5,0],[-1,.5,0]]]]
        try:
            pieces=list(G.Brep.CreatePlanarBreps(polygons,1e-6)or[])
            if len(pieces)!=1:raise ValueError('one owned planar trimmed face required')
            surface=pieces[0]
        finally:
            for p in polygons:p.Dispose()
    ids=[];layers=[];old_layer=doc.Layers.CurrentLayerIndex
    def snapshot():
        rows=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=o.Geometry;role='source'if o.Id==ids[0]else'target'if o.Id==ids[1]else'output'
            row=dict(role=role,selected=bool(o.IsSelected(False)),layer=doc.Layers[o.Attributes.LayerIndex].Name,name=o.Attributes.Name,crc=int(g.DataCRC(0)))
            if isinstance(g,G.Curve):row['curve']=host['_nurbs_curve_definition'](g)
            rows.append(row)
        return rows
    try:
        record=doc.BeginUndoRecord('Owned surface offset scene')
        try:
            for name in ['Surface offset source','Surface offset output']:
                layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer))
            a=R.DocObjects.ObjectAttributes();a.Name='Source';a.LayerIndex=layers[0];ids.append(doc.Objects.AddCurve(source,a))
            a=R.DocObjects.ObjectAttributes();a.Name='Target';ids.append(doc.Objects.AddBrep(surface,a))
            doc.Layers.SetCurrentLayerIndex(layers[1],True)
            if op['case']!='postselect':doc.Objects.Select(ids[0])
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);before=snapshot();traces=[];marker='Owned surface offset '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        def execute():
            result,trace=drive(op,host,ids,'OffsetCrvOnSrf');traces.append(trace);return result
        success,after,events=observe_command(R.Commands.Command,'OffsetCrvOnSrf',execute,snapshot,lambda:[],True)
        after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,input_trace=traces[0],events=events,history=history),0
    finally:
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
        source.Dispose();surface.Dispose()

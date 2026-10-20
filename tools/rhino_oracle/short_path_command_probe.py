"""Fixed public ShortPath getter scenes in an empty owned document."""
import re
from copy import deepcopy
CASES={'plane':[[-4.,-3.,0.],[4.,3.,0.]],'cancel':[[-4.,-3.,0.],'Cancel'],'cylinder':[[3.,0.,1.],[0.,3.,5.]],'general':[[.4,.5,.05],[1.6,1.5,.84]]}
BOUNDARY_CASES={
    'hole_horizontal':[[-4.,0.,0.],[4.,0.,0.]],
    'hole_upper':[[-4.,.75,0.],[4.,.75,0.]],
    'hole_clear':[[-4.,3.,0.],[4.,3.,0.]],
    'hole_start_in_hole':[[0.,0.,0.],[4.,0.,0.]],
    'concave':[[.5,3.5,0.],[3.5,.5,0.]],
    'concave_reverse':[[3.5,.5,0.],[.5,3.5,0.]],
    'hole_tilted':[[-4.,0.,0.],[4.,0.,0.]],
    'hole_reversed':[[-4.,0.,0.],[4.,0.,0.]],
}
def boundary_request():
    q=request();q['operations']=[dict(op='short_path_command',id=k,case=k,actions=deepcopy(v))for k,v in BOUNDARY_CASES.items()];return q
def boundary_source(case,host):
    G=host['Rhino'].Geometry
    loops=[[[0.,0.,0.],[4.,0.,0.],[4.,1.,0.],[1.,1.,0.],[1.,4.,0.],[0.,4.,0.],[0.,0.,0.]]]if case.startswith('concave')else[[[-5.,-5.,0.],[5.,-5.,0.],[5.,5.,0.],[-5.,5.,0.],[-5.,-5.,0.]],[[-1.,-1.,0.],[-1.,1.,0.],[1.,1.,0.],[1.,-1.,0.],[-1.,-1.,0.]]]
    curves=[G.PolylineCurve([G.Point3d(*p)for p in loop])for loop in loops]
    try:
        breps=G.Brep.CreatePlanarBreps(curves,1e-9)
        if breps is None or len(breps)!=1:raise ValueError('owned planar ShortPath boundaries failed')
        brep=breps[0]
        if case=='hole_tilted':
            transform=G.Transform.Rotation(.6,G.Vector3d.XAxis,G.Point3d.Origin)
            if not brep.Transform(transform):brep.Dispose();raise ValueError('owned boundary tilt failed')
            loops=[[host['_xyz'](transform*G.Point3d(*p))for p in loop]for loop in loops]
        if case=='hole_reversed':brep.Flip()
        if not brep.IsValid:brep.Dispose();raise ValueError('invalid owned ShortPath face')
        return brep,loops
    finally:
        for curve in curves:curve.Dispose()
def request():
    return deepcopy(dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='short_path_command',id=k,case=k,actions=v)for k,v in CASES.items()]))
def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','case','actions'}or op['op']!='short_path_command'or not isinstance(op['case'],str)or op['case']not in dict(CASES,**BOUNDARY_CASES)or op['actions']!=dict(CASES,**BOUNDARY_CASES)[op['case']]or not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id'])is None:raise ValueError('fixed owned ShortPath scenes required')
def validate_request(q):
    if type(q.get('iterations'))is not int or q['iterations']!=1 or not isinstance(q.get('operations'),list)or not 1<=len(q['operations'])<=8:raise ValueError('bounded ShortPath command request required')
    for op in q['operations']:validate(op)
    if len({op['id']for op in q['operations']})!=len(q['operations']):raise ValueError('unique ids required')
def run(op,host,iterations,tolerance):
    validate(op)
    from surface_short_path_probe import source
    from join_probe import observe_command
    from conic_getter_input import drive
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('empty owned idle document required')
    surface,boundaries=boundary_source(op['case'],host)if op['case']in BOUNDARY_CASES else(source('plane'if op['case']=='cancel'else op['case'],host),None)
    old=doc.Layers.CurrentLayerIndex;layers=[];identifier=None
    def snapshot():
        rows=[]
        for o in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            row=dict(role='target'if o.Id==identifier else'output',selected=bool(o.IsSelected(False)),layer=doc.Layers[o.Attributes.LayerIndex].Name,name=o.Attributes.Name,crc=int(o.Geometry.DataCRC(0)))
            if isinstance(o.Geometry,G.Curve):row['curve']=host['_nurbs_curve_definition'](o.Geometry)
            rows.append(row)
        return rows
    try:
        record=doc.BeginUndoRecord('Owned ShortPath source')
        try:
            for name in ['ShortPath source','ShortPath output']:
                layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer))
            a=R.DocObjects.ObjectAttributes();a.LayerIndex=layers[0];a.Name='Target';identifier=doc.Objects.AddBrep(surface,a)if isinstance(surface,G.Brep)else doc.Objects.AddSurface(surface,a)
            doc.Layers.SetCurrentLayerIndex(layers[1],True);doc.Objects.Select(identifier)
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);before=snapshot();traces=[]
        marker='Owned ShortPath '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        def execute():
            success,trace=drive(op,host,[identifier],'ShortPath');traces.append(trace);return success
        success,after,events=observe_command(R.Commands.Command,'ShortPath',execute,snapshot,lambda:[],True)
        after_script=snapshot();history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(boundaries=boundaries,surface=host['_nurbs_surface_definition'](surface.Faces[0].UnderlyingSurface()if isinstance(surface,G.Brep)else surface),success=success,before=before,after=after,after_script=after_script,undo=undo,redo=redo,input_trace=traces[0],events=events,history=history),0
    finally:
        doc.Layers.SetCurrentLayerIndex(old,True)
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for index in reversed(layers):doc.Layers.Delete(index,True)
        surface.Dispose()

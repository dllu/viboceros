"""Fixed public MeshPatch command and SDK inputs, in an owned idle document."""
from copy import deepcopy
import math

CASES = {
    'points': dict(points=[[0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[0.,4.,0.],[2.,1.,0.]]),
    'raised': dict(points=[[0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[0.,4.,0.],[2.,1.,2.]]),
    'tilted': dict(points=[[0.,0.,0.],[5.,0.,5.],[5.,4.,7.],[0.,4.,2.],[2.,1.,2.5]]),
    'concave': dict(curves=[[[0.,0.,0.],[5.,0.,0.],[5.,1.,0.],[2.,1.,0.],[2.,4.,0.],[0.,4.,0.],[0.,0.,0.]]],points=[[1.,1.,0.]]),
    'hole': dict(curves=[[[0.,0.,0.],[6.,0.,0.],[6.,5.,0.],[0.,5.,0.],[0.,0.,0.]]],holes=[[[2.,1.,0.],[4.,1.,0.],[4.,3.,0.],[2.,3.,0.],[2.,1.,0.]]]),
    'constraint': dict(points=[[0.,0.,0.],[5.,0.,0.],[4.,4.,0.],[0.,3.,0.]],curves=[[[1.,1.,0.],[3.,2.,0.]]]),
    'duplicate_constraint': dict(points=[[0.,0.,0.],[5.,0.,0.],[4.,4.,0.],[0.,3.,0.]],curves=[[[0.,0.,0.],[4.,4.,0.]]],expect_failure=True),
    'untrimmed': dict(points=[[0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[0.,4.,0.],[2.,1.,0.]],trimback=False,divisions=2),
    'line3': dict(points=[[0.,0.,0.],[5.,0.,0.],[4.,4.,0.],[0.,3.,0.]],curves=[[[1.,1.,0.],[2.,3.,0.],[3.,2.,0.]]]),
    'guide_with_outer': dict(curves=[[[0.,0.,0.],[6.,0.,0.],[6.,5.,0.],[0.,5.,0.],[0.,0.,0.]],[[1.,1.,0.],[3.,2.,0.]]]),
    'inner_closed': dict(points=[[0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[0.,4.,0.]],curves=[[[1.,1.,0.],[3.,1.,0.],[3.,2.,0.],[1.,2.,0.],[1.,1.,0.]]]),
    'untrimmed_raised': dict(points=[[0.,0.,0.],[5.,0.,0.],[5.,4.,0.],[0.,4.,0.],[2.,1.,2.]],trimback=False,divisions=2),
}
for _name in ['constraint','concave','inner_closed']:
    CASES['untrimmed_'+_name]=dict(deepcopy(CASES[_name]),trimback=False,divisions=1)
for _name,_source,_point in [('filled_inner','inner_closed',[2.,1.5,0.]),('filled_open3','line3',[2.,2.,0.]),('concave_outside','concave',[3.,2.,0.])]:
    CASES[_name]=deepcopy(CASES[_source]);CASES[_name]['points'].append(_point)
CASES['circle']=dict(circle=[0.,0.,0.,3.])
CASES['cloud']=dict(deepcopy(CASES['raised']),cloud=True)
CASES['surface']=dict(points=[[1.,1.,.1],[4.,1.,.4],[4.,3.,1.2],[1.,3.,.3],[2.,2.,.4]],surface=[[0.,0.,0.],[5.,0.,0.],[5.,4.,2.],[0.,4.,0.]],trimback=False,divisions=2)

def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='mesh_patch_command',id=name,case=name)for name in CASES])

def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','case'}or op['op']!='mesh_patch_command'or op['case']not in CASES or op['id']!=op['case']:
        raise ValueError('fixed MeshPatch case required')

def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:
        raise ValueError('empty owned idle document required')
    case=deepcopy(CASES[op['case']]);points=[G.Point3d(*p)for p in case.get('points',[])]
    curves=[G.PolylineCurve([G.Point3d(*p)for p in line])for line in case.get('curves',[])]
    if 'circle'in case:
        c=case['circle'];curves.append(G.Circle(G.Point3d(*c[:3]),c[3]).ToNurbsCurve())
    holes=[G.PolylineCurve([G.Point3d(*p)for p in line])for line in case.get('holes',[])]
    surface=G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p)for p in case['surface']])if 'surface'in case else None
    surface_id=None
    sources=[];hole_ids=[];old_layer=doc.Layers.CurrentLayerIndex;layers=[]
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            row=dict(role='reference'if obj.Id==surface_id else'source'if obj.Id in sources or obj.Id in hole_ids else'output',selected=bool(obj.IsSelected(False)),layer=doc.Layers[obj.Attributes.LayerIndex].Name,name=obj.Attributes.Name,crc=int(obj.Geometry.DataCRC(0)))
            if isinstance(obj.Geometry,G.Mesh):row['mesh']=host['_polygon_mesh_value'](obj.Geometry)
            if isinstance(obj.Geometry,G.Point):row['point']=host['_xyz'](obj.Geometry.Location)
            if isinstance(obj.Geometry,G.PointCloud):row['points']=[host['_xyz'](p)for p in obj.Geometry.GetPoints()]
            rows.append(row)
        return rows
    try:
        # Record the public SDK and command separately: closed and open guides
        # can affect trimming even without the SDK's explicit outer boundary.
        sdk=G.Mesh.CreatePatch(None,math.pi/12.,surface,holes,curves,points,case.get('trimback',True),case.get('divisions',1))
        sdk_value=host['_polygon_mesh_value'](sdk)if sdk else None
        if sdk:sdk.Dispose()
        record=doc.BeginUndoRecord('Owned MeshPatch source')
        try:
            for name in ['MeshPatch source','MeshPatch output']:
                layer=R.DocObjects.Layer();layer.Name=name;layers.append(doc.Layers.Add(layer))
            attributes=R.DocObjects.ObjectAttributes();attributes.LayerIndex=layers[0];attributes.Name='Input'
            if case.get('cloud'):sources.append(doc.Objects.AddPointCloud(points,attributes))
            else:
                for point in points:sources.append(doc.Objects.AddPoint(point,attributes))
            for curve in curves:sources.append(doc.Objects.AddCurve(curve,attributes))
            for curve in holes:hole_ids.append(doc.Objects.AddCurve(curve,attributes))
            if surface:surface_id=doc.Objects.AddSurface(surface,attributes)
            doc.Layers.SetCurrentLayerIndex(layers[1],True)
            for identifier in sources:doc.Objects.Select(identifier)
        finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);before=snapshot()
        marker='Owned MeshPatch '+str(host['System'].Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        command='_-MeshPatch _TrimBack='+('Yes'if case.get('trimback',True)else'No')+' _AngleTolerance=15'
        if not case.get('trimback',True):command+=' _MainSegmentCount='+str(case.get('divisions',1))
        if surface_id:command+=' _StartingSurface _SelID '+str(surface_id)
        for identifier in hole_ids:command+=' _SelID '+str(identifier)
        command+=' _Enter'
        success,after,events=observe_command(R.Commands.Command,'MeshPatch',lambda:R.RhinoApp.RunScript(command,True),snapshot,lambda:[],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();after_script=snapshot()
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(input=case,sdk=sdk_value,surface=host['_nurbs_surface_definition'](surface)if surface else None,sdk_signature=str(G.Mesh.CreatePatch.__doc__),success=success,events=events,history=history,before=before,after=after,after_script=after_script,undo=undo,redo=redo),0
    finally:
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
        for curve in curves+holes:curve.Dispose()
        if surface:surface.Dispose()

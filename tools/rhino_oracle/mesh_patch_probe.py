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

_OUTER=[[-4.,-3.,0.],[4.,-3.,0.],[4.,3.,0.],[-4.,3.,0.],[-4.,-3.,0.]]
_AXES=[[[-3.,0.,0.],[3.,0.,0.]],[[0.,-2.,0.],[0.,2.,0.]]]
CROSSING_CASES={
    'cross_axes':dict(curves=[deepcopy(_OUTER)]+deepcopy(_AXES)),
    'cross_diagonals':dict(curves=[deepcopy(_OUTER),[[-3.,-2.,0.],[3.,2.,0.]],[[-3.,2.,0.],[3.,-2.,0.]]]),
    'shared_endpoint':dict(curves=[deepcopy(_OUTER),[[-3.,0.,0.],[0.,0.,0.]],[[0.,0.,0.],[0.,2.,0.]],[[0.,0.,0.],[3.,-1.,0.]]]),
    'point_at_crossing':dict(points=[[0.,0.,0.]],curves=[deepcopy(_OUTER)]+deepcopy(_AXES)),
    'overlap':dict(curves=[deepcopy(_OUTER),[[-3.,0.,0.],[1.,0.,0.]],[[-1.,0.,0.],[3.,0.,0.]]]),
    'self_cross':dict(curves=[deepcopy(_OUTER),[[-3.,-2.,0.],[3.,2.,0.],[-3.,2.,0.],[3.,-2.,0.],[-3.,-2.,0.]]]),
    'hole_cut':dict(curves=[deepcopy(_OUTER),deepcopy(_AXES[0])],holes=[[[-1.,-1.,0.],[1.,-1.,0.],[1.,1.,0.],[-1.,1.,0.],[-1.,-1.,0.]]]),
    'cross_3d':dict(curves=[deepcopy(_OUTER),[[-3.,0.,1.],[3.,0.,1.]],[[0.,-2.,-1.],[0.,2.,-1.]]]),
    'cross_3d_reverse':dict(curves=[deepcopy(_OUTER),[[0.,-2.,-1.],[0.,2.,-1.]],[[-3.,0.,1.],[3.,0.,1.]]]),
    'cross_lifted':dict(curves=[deepcopy(_OUTER),[[-3.,0.,1.],[3.,0.,1.]],[[0.,-2.,1.],[0.,2.,1.]]]),
    'cross_sloped':dict(curves=[deepcopy(_OUTER),[[-3.,-2.,0.],[3.,2.,2.]],[[-2.,2.,3.],[1.,-1.,0.]]]),
}
SCENES=CASES.copy();SCENES.update(CROSSING_CASES)
CROSSING_VARIANTS={
    'cross_v_reverse':dict(curves=[deepcopy(_OUTER),deepcopy(_AXES[0]),list(reversed(deepcopy(_AXES[1])))]),
    'cross_h_reverse':dict(curves=[deepcopy(_OUTER),list(reversed(deepcopy(_AXES[0]))),deepcopy(_AXES[1])]),
    'cross_both_reverse':dict(curves=[deepcopy(_OUTER),list(reversed(deepcopy(_AXES[0]))),list(reversed(deepcopy(_AXES[1])))]),
    'cross_v_first':dict(curves=[deepcopy(_OUTER),deepcopy(_AXES[1]),deepcopy(_AXES[0])]),
    'cross_asymmetric':dict(curves=[deepcopy(_OUTER),[[-3.,-.4,0.],[3.,1.2,0.]],[[-1.,-2.,0.],[1.5,2.,0.]]]),
    'two_crossings':dict(curves=[deepcopy(_OUTER),deepcopy(_AXES[0]),[[-1.,-2.,0.],[-1.,2.,0.]],[[1.,-2.,0.],[1.,2.,0.]]]),
    'cross_seed':dict(points=[[2.,1.,0.]],curves=[deepcopy(_OUTER)]+deepcopy(_AXES)),
    'cross_untrimmed':dict(curves=[deepcopy(_OUTER)]+deepcopy(_AXES),trimback=False,divisions=1),
    'hole_cut_reverse':dict(curves=[deepcopy(_OUTER),list(reversed(deepcopy(_AXES[0])))],holes=deepcopy(CROSSING_CASES['hole_cut']['holes'])),
    'hole_cut_raised':dict(curves=[deepcopy(_OUTER),[[-3.,0.,1.],[3.,0.,1.]]],holes=deepcopy(CROSSING_CASES['hole_cut']['holes'])),
}
SCENES.update(CROSSING_VARIANTS)
UNTRIMMED_CROSSING_CASES={}
for _name in ['cross_axes','cross_diagonals','shared_endpoint','overlap','self_cross','cross_3d','cross_3d_reverse','cross_lifted','cross_sloped']:
    UNTRIMMED_CROSSING_CASES[_name+'_untrimmed']=dict(deepcopy(CROSSING_CASES[_name]),trimback=False,divisions=1)
SCENES.update(UNTRIMMED_CROSSING_CASES)
SEEDED_CROSSING_CASES={
    'cross_seed_top':dict(points=[[2.,1.,0.]],curves=[deepcopy(_OUTER)]+deepcopy(_AXES)),
    'cross_seed_bottom':dict(points=[[2.,-1.,0.]],curves=[deepcopy(_OUTER)]+deepcopy(_AXES)),
    'cross_seed_left':dict(points=[[-2.,1.,0.]],curves=[deepcopy(_OUTER)]+deepcopy(_AXES)),
    'cross_seed_outside':dict(points=[[6.,1.,0.]],curves=[deepcopy(_OUTER)]+deepcopy(_AXES)),
    'cross_seed_axis':dict(points=[[1.,0.,0.]],curves=[deepcopy(_OUTER)]+deepcopy(_AXES)),
    'cross_seed_3d':dict(points=[[2.,1.,.2]],curves=deepcopy(CROSSING_CASES['cross_3d']['curves'])),
    'cross_seed_cloud':dict(points=[[2.,1.,0.],[-2.,-1.,0.]],cloud=True,curves=[deepcopy(_OUTER)]+deepcopy(_AXES)),
}
SCENES.update(SEEDED_CROSSING_CASES)
JUNCTION_CASES={name:deepcopy(SCENES[name])for name in ['shared_endpoint','overlap','cross_seed_axis','two_crossings']}
_LEFT=[-3.,0.,0.];_CENTER=[0.,0.,0.];_TOP=[0.,2.,0.];_BRANCH=[3.,-1.,0.]
for _name,_curves in [
    ('junction_two',[[_LEFT,_CENTER],[_CENTER,_TOP]]),
    ('junction_two_out',[[_CENTER,_LEFT],[_CENTER,_TOP]]),
    ('junction_three_out',[[_CENTER,_LEFT],[_CENTER,_TOP],[_CENTER,_BRANCH]]),
    ('junction_three_in',[[_LEFT,_CENTER],[_TOP,_CENTER],[_BRANCH,_CENTER]]),
    ('junction_three_reverse',[[_LEFT,_CENTER],[_TOP,_CENTER],[_CENTER,_BRANCH]]),
    ('junction_chain',[[_LEFT,_CENTER,_TOP]]),
    ('junction_chain_branch',[[_LEFT,_CENTER,_TOP],[_CENTER,_BRANCH]]),
    ('junction_t',[[_LEFT,[3.,0.,0.]],[_CENTER,_TOP]]),
    ('junction_t_reverse',[[_LEFT,[3.,0.,0.]],[_TOP,_CENTER]]),
]:
    JUNCTION_CASES[_name]=dict(curves=[deepcopy(_OUTER)]+deepcopy(_curves))
for _name,_point in [('junction_seed_center',_CENTER),('junction_seed_side',[2.,1.,0.])]:
    JUNCTION_CASES[_name]=dict(deepcopy(CROSSING_CASES['shared_endpoint']),points=[deepcopy(_point)])
for _name,_curves in [
    ('overlap_order',list(reversed(CROSSING_CASES['overlap']['curves'][1:]))),
    ('overlap_reverse',[CROSSING_CASES['overlap']['curves'][1],list(reversed(CROSSING_CASES['overlap']['curves'][2]))]),
    ('overlap_nested',[[_LEFT,[3.,0.,0.]],[[-1.,0.,0.],[1.,0.,0.]]]),
    ('overlap_nested_reverse',[[_LEFT,[3.,0.,0.]],[[1.,0.,0.],[-1.,0.,0.]]]),
    ('overlap_touch',[[_LEFT,[-1.,0.,0.]],[[-1.,0.,0.],[3.,0.,0.]]]),
    ('overlap_duplicate',deepcopy([_AXES[0],_AXES[0]])),
    ('overlap_duplicate_reverse',deepcopy([_AXES[0],list(reversed(_AXES[0]))])),
]:
    JUNCTION_CASES[_name]=dict(curves=[deepcopy(_OUTER)]+deepcopy(_curves))
JUNCTION_CASES['cross_seed_axis_left']=dict(deepcopy(SCENES['cross_seed_axis']),points=[[-1.,0.,0.]])
JUNCTION_CASES['cross_seed_axis_reverse']=deepcopy(SCENES['cross_seed_axis']);JUNCTION_CASES['cross_seed_axis_reverse']['curves'][1].reverse()
SCENES.update(JUNCTION_CASES)
AUDIT_CASES={name:deepcopy(SCENES[name])for name in ['shared_endpoint','overlap','cross_axes','cross_3d','self_cross','cross_seed_axis','two_crossings','junction_two','junction_chain','overlap_duplicate','overlap_touch']}
for _name,_case in list(AUDIT_CASES.items()):AUDIT_CASES[_name+'_sdk_after']=dict(deepcopy(_case),sdk_after=True)
SCENES.update(AUDIT_CASES)
POINT_GUIDE_CASES={
    'point_guide_quarter':dict(deepcopy(SCENES['cross_seed_axis']),points=[[2.,0.,0.]]),
    'point_guide_diagonal':dict(deepcopy(SCENES['cross_diagonals']),points=[[-1.5,-1.,0.]]),
    'point_guide_cloud':dict(deepcopy(SCENES['cross_seed_axis']),cloud=True),
    'point_guide_two':dict(deepcopy(SCENES['cross_seed_axis']),points=[[-1.,0.,0.],[1.,0.,0.]]),
    'point_guide_raised':dict(deepcopy(SCENES['cross_3d']),points=[[1.,0.,1.]]),
    'overlap_first_reversed':deepcopy(SCENES['overlap']),
    'overlap_shifted':deepcopy(SCENES['overlap']),
    'overlap_tilted':deepcopy(SCENES['overlap']),
    'overlap_seed_above':dict(deepcopy(SCENES['overlap']),points=[[0.,1.,0.]]),
    'overlap_seed_below':dict(deepcopy(SCENES['overlap']),points=[[0.,-1.,0.]]),
    'overlap_vertical':deepcopy(SCENES['overlap']),
    'point_guide_tilted':deepcopy(SCENES['cross_seed_axis']),
}
POINT_GUIDE_CASES['overlap_first_reversed']['curves'][1].reverse()
for _name,_transform in [
    ('overlap_shifted',lambda x,y,z:[x+1024.,y-2048.,z+512.]),
    ('overlap_tilted',lambda x,y,z:[x,y,z+.5*x+.25*y]),
    ('overlap_vertical',lambda x,y,z:[-y,x,z]),
    ('point_guide_tilted',lambda x,y,z:[x,y,z+.5*x+.25*y]),
]:
    _case=POINT_GUIDE_CASES[_name]
    _case['curves']=[[_transform(*p)for p in c]for c in _case['curves']]
    if 'points'in _case:_case['points']=[_transform(*p)for p in _case['points']]
SCENES.update(POINT_GUIDE_CASES)
def crossing_request():
    q=request();q['operations']=[dict(op='mesh_patch_command',id=name,case=name)for name in CROSSING_CASES];return q
def crossing_verify_request():
    q=crossing_request();q['operations']+=[dict(op='mesh_patch_command',id=name,case=name)for name in CROSSING_VARIANTS];return q
def untrimmed_crossing_request():
    q=request();q['operations']=[dict(op='mesh_patch_command',id=name,case=name)for name in UNTRIMMED_CROSSING_CASES];return q
def seeded_crossing_request():
    q=request();q['operations']=[dict(op='mesh_patch_command',id=name,case=name)for name in SEEDED_CROSSING_CASES];return q
def junction_request():
    q=request();q['operations']=[dict(op='mesh_patch_command',id=name,case=name)for name in JUNCTION_CASES];return q
def audit_request():
    q=request();q['operations']=[dict(op='mesh_patch_command',id=name,case=name)for name in AUDIT_CASES];return q
def point_guide_request():
    q=request();q['operations']=[dict(op='mesh_patch_command',id=name,case=name)for name in POINT_GUIDE_CASES];return q

def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='mesh_patch_command',id=name,case=name)for name in CASES])

def validate(op):
    if not isinstance(op,dict)or set(op)!={'op','id','case'}or op['op']!='mesh_patch_command'or op['case']not in SCENES or op['id']!=op['case']:
        raise ValueError('fixed MeshPatch case required')

def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:
        raise ValueError('empty owned idle document required')
    case=deepcopy(SCENES[op['case']]);points=[G.Point3d(*p)for p in case.get('points',[])]
    curves=[G.PolylineCurve([G.Point3d(*p)for p in line])for line in case.get('curves',[])]
    if 'circle'in case:
        c=case['circle'];curves.append(G.Circle(G.Point3d(*c[:3]),c[3]).ToNurbsCurve())
    holes=[G.PolylineCurve([G.Point3d(*p)for p in line])for line in case.get('holes',[])]
    surface=G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p)for p in case['surface']])if 'surface'in case else None
    surface_id=None
    sources=[];hole_ids=[];old_layer=doc.Layers.CurrentLayerIndex;layers=[]
    def curve_inputs():
        rows=[]
        for curve in curves+holes:
            ok,polyline=curve.TryGetPolyline()
            rows.append(dict(crc=int(curve.DataCRC(0)),polyline=[host['_xyz'](p)for p in polyline]if ok else None))
        return rows
    def sdk_probe():
        before=curve_inputs()
        sdk=G.Mesh.CreatePatch(None,math.pi/12.,surface,holes,curves,points,case.get('trimback',True),case.get('divisions',1))
        try:value=host['_polygon_mesh_value'](sdk)if sdk else None
        finally:
            if sdk:sdk.Dispose()
        return value,dict(before=before,after=curve_inputs())
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            row=dict(role='reference'if obj.Id==surface_id else'source'if obj.Id in sources or obj.Id in hole_ids else'output',selected=bool(obj.IsSelected(False)),layer=doc.Layers[obj.Attributes.LayerIndex].Name,name=obj.Attributes.Name,crc=int(obj.Geometry.DataCRC(0)))
            if isinstance(obj.Geometry,G.Mesh):row['mesh']=host['_polygon_mesh_value'](obj.Geometry)
            if isinstance(obj.Geometry,G.Point):row['point']=host['_xyz'](obj.Geometry.Location)
            if isinstance(obj.Geometry,G.PointCloud):row['points']=[host['_xyz'](p)for p in obj.Geometry.GetPoints()]
            if isinstance(obj.Geometry,G.Curve):
                ok,polyline=obj.Geometry.TryGetPolyline();row['polyline']=[host['_xyz'](p)for p in polyline]if ok else None
            rows.append(row)
        return rows
    try:
        # Record the public SDK and command separately: closed and open guides
        # can affect trimming even without the SDK's explicit outer boundary.
        sdk_value=None;sdk_inputs=None
        if not case.get('sdk_after'):sdk_value,sdk_inputs=sdk_probe()
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
        if case.get('sdk_after'):sdk_value,sdk_inputs=sdk_probe()
        return dict(input=case,sdk=sdk_value,sdk_inputs=sdk_inputs,surface=host['_nurbs_surface_definition'](surface)if surface else None,sdk_signature=str(G.Mesh.CreatePatch.__doc__),success=success,events=events,history=history,before=before,after=after,after_script=after_script,undo=undo,redo=redo),0
    finally:
        doc.Layers.SetCurrentLayerIndex(old_layer,True)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        for layer in reversed(layers):doc.Layers.Delete(layer,True)
        for curve in curves+holes:curve.Dispose()
        if surface:surface.Dispose()

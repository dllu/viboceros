"""Fixed creator representation, surface parameterization and ToSubD workflows."""
from copy import deepcopy

def yes(value):
    return 'Yes' if value else 'No'


CASES = {
    'creator_detail_box': dict(command='Box', macro='_Box w0,0,0 w4,3,0 2', application='Box 0,0,0 4,3,0 2'),
    'creator_detail_box_negative': dict(command='Box', macro='_Box w0,0,0 w4,3,0 -2', application='Box 0,0,0 4,3,0 -2'),
    'creator_detail_box_reversed': dict(command='Box', macro='_Box w4,3,1 w0,0,1 2', application='Box 4,3,1 0,0,1 2'),
    'creator_detail_box_tilted': dict(command='Box', macro='_Box w10,20,30 w12.4,23.2,33 -2', application='Box 10,20,30 12.4,23.2,33 -2', construction_plane=[[10.,20.,30.],[.6,.8,0.],[0.,0.,1.]]),
    'creator_detail_cylinder_default': dict(command='Cylinder', macro='_Cylinder w0,0,0 2 3', application='Cylinder 0,0,0 2 3'),
}
for solid in [False,True]:
    for both in [False,True]:
        for height in [-3.,3.]:
            key='creator_detail_cylinder_'+str(int(solid))+str(int(both))+('n'if height<0 else'p')
            CASES[key]=dict(command='Cylinder',macro='_Cylinder _Solid=_'+yes(solid)+' w1,2,3 2 _BothSides=_'+yes(both)+' '+str(height),application='Cylinder 1,2,3 2 '+str(height)+' Solid='+yes(solid)+' BothSides='+yes(both))
for profile in ['rectangle','circle','open_kink']:
    for solid in [False,True]:
        for both in [False,True]:
            for height in [-2.,2.]:
                key='creator_detail_extrude_'+profile+'_'+str(int(solid))+str(int(both))+('n'if height<0 else'p')
                CASES[key]=dict(command='ExtrudeCrv',profile=profile,macro='_-ExtrudeCrv _Output=_Surface _Solid=_'+yes(solid)+' _BothSides=_'+yes(both)+' _DeleteInput=_No '+str(height),application='ExtrudeCrv '+str(height)+' Solid='+yes(solid)+' BothSides='+yes(both)+' DeleteInput=No')
for profile in ['xz','line','nonplanar']:
    CASES['creator_detail_extrude_'+profile]=dict(command='ExtrudeCrv',profile=profile,macro='_-ExtrudeCrv _Output=_Surface _Solid=_No _BothSides=_No _DeleteInput=_No -2',application='ExtrudeCrv -2 Solid=No BothSides=No DeleteInput=No')
for profile in ['rectangle','circle']:
    CASES['creator_detail_oblique_'+profile]=dict(command='ExtrudeCrv',profile=profile,macro='_-ExtrudeCrv _Output=_Surface _Solid=_Yes _BothSides=_No _DeleteInput=_No _Direction w0,0,0 w1,2,3 '+'3.74165738677394',application='ExtrudeCrv 0,0,0 1,2,3 Solid=Yes BothSides=No DeleteInput=No')


def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='creator_extrusions',id=k,case=k)for k in CASES])


def validate(op):
    try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='creator_extrusions'and op['case']in CASES and op['id']==op['case']
    except TypeError:valid=False
    if not valid:raise ValueError('fixed creator extrusion recipe required')


def run(op,host,iterations,tolerance):
    validate(op)
    from join_probe import observe_command
    from subd_box_probe import subd_value
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc;vp=doc.Views.ActiveView.ActiveViewport
    if iterations!=1 or list(doc.Objects)or doc.UndoRecordingIsActive:raise ValueError('empty owned idle document required')
    case=deepcopy(CASES[op['case']]);sources=[];saved_plane=vp.GetConstructionPlane()
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=obj.Geometry;row=dict(source=obj.Id in sources,type=str(g.GetType().Name),selected=bool(obj.IsSelected(False)),crc=int(g.DataCRC(0)))
            if isinstance(g,G.SubD):row['subd']=subd_value(g,host)
            elif isinstance(g,G.Curve):row['curve']=host['_nurbs_curve_definition'](g)
            elif isinstance(g,(G.Brep,G.Surface)):
                row['solid']=bool(g.IsSolid) if isinstance(g,(G.Brep,G.Extrusion)) else False
                if isinstance(g,G.Extrusion):
                    row['extrusion']=dict(caps=[bool(g.IsCappedAtBottom),bool(g.IsCappedAtTop)],profiles=int(g.ProfileCount),path_start=host['_xyz'](g.PathStart),path_end=host['_xyz'](g.PathEnd),surface=host['_nurbs_surface_definition'](g))
                    brep=g.ToBrep()
                elif isinstance(g,G.Brep):brep=g
                else:brep=g.ToBrep()
                if brep is None:raise ValueError('missing canonical B-rep')
                try:
                    row['faces']=[dict(surface=host['_nurbs_surface_definition'](f.UnderlyingSurface()),reversed=bool(f.OrientationIsReversed),loops=int(f.Loops.Count))for f in brep.Faces]
                finally:
                    if brep is not g:brep.Dispose()
            else:raise ValueError('unexpected creator output')
            rows.append(row)
        return rows
    try:
        if 'construction_plane'in case:
            origin,x,y=case['construction_plane'];vp.SetConstructionPlane(G.Plane(G.Point3d(*origin),G.Vector3d(*x),G.Vector3d(*y)))
        if 'profile'in case:
            record=doc.BeginUndoRecord('Owned creator profile')
            try:
                name=case['profile'];plane=G.Plane.WorldXY
                if name=='rectangle':curve=G.Rectangle3d(plane,4.,3.).ToNurbsCurve()
                elif name=='circle':curve=G.Circle(plane,2.).ToNurbsCurve()
                elif name=='line':curve=G.LineCurve(G.Point3d(0.,0.,0.),G.Point3d(3.,0.,0.))
                elif name=='nonplanar':curve=G.NurbsCurve.Create(False,3,[G.Point3d(0.,0.,0.),G.Point3d(1.,2.,1.),G.Point3d(2.,1.,3.),G.Point3d(4.,0.,0.)])
                else:curve=G.PolylineCurve([G.Point3d(0.,0.,0.),G.Point3d(1.,0.,1.)if name=='xz'else G.Point3d(1.,1.,0.),G.Point3d(3.,0.,0.)])
                try:sources.append(doc.Objects.AddCurve(curve))
                finally:curve.Dispose()
                doc.Objects.Select(sources[0])
            finally:doc.EndUndoRecord(record)
        doc.ClearUndoRecords(True);before=snapshot();marker='Owned creator details '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro=case['macro']+' _Cancel _Cancel _Cancel'
        success,created,events=observe_command(R.Commands.Command,case['command'],lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[1].strip();after_script=snapshot()
        if not success or len([r for r in created if not r['source']])!=1 or 'Unknown command:'in history:raise ValueError('creator failed: '+history)
        R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        output=next(obj for obj in doc.Objects if obj.Id not in sources)
        eligible=isinstance(output.Geometry,G.Extrusion)or(isinstance(output.Geometry,G.Brep)and output.Geometry.Faces.Count==1)
        converted=None;after=None;conversion_events=[]
        if eligible:
            doc.Objects.UnselectAll();doc.Objects.Select(output.Id)
            converted,after,conversion_events=observe_command(R.Commands.Command,'ToSubD',lambda:R.RhinoApp.RunScript('_-ToSubD _UseMesh=_ControlPoints _MeshCreases=_No _MeshCorners=_No _UseSurface=_Location _SurfaceCorners=_No _DeleteInput=_No _Enter _Cancel _Cancel _Cancel',True),snapshot,lambda:[],True)
        return dict(input=case,success=success,before=before,created=created,events=events,history=history,after_script=after_script,undo=undo,redo=redo,tosubd_success=converted,after=after,conversion_events=conversion_events),0
    finally:
        vp.SetConstructionPlane(saved_plane)
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)

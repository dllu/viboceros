"""Closed public SubCrv numeric/reference getter diagnostics."""
import re

CASES=('point_control','number_only','number_reference','standalone_reference',
       'number_click','standalone_click','empty_nested','zero',
       'confirm_coordinates','confirm_reverse','confirm_negative','confirm_quadratic',
       'confirm_closed_forward','confirm_closed_backward','confirm_closed_full','confirm_unavailable',
       'confirm_tail','negative_forward','negative_backward','negative_closed','replace_number','confirm_empty',
       'confirm_nonuniform','confirm_clamp','confirm_same',
       'apply_forward','apply_reverse','apply_closed','apply_clamp')
def request():
    return dict(protocol_version=1,iterations=1,operations=[dict(op='subcurve_numeric_followup',id='numeric_followup_'+c,case=c) for c in CASES])
def validate_request(q):
    if (not isinstance(q,dict) or type(q.get('protocol_version')) is not int or q['protocol_version']!=1
            or type(q.get('iterations',1)) is not int or q.get('iterations',1)!=1
            or not isinstance(q.get('operations'),list) or not 1<=len(q['operations'])<=32):
        raise ValueError('numeric followup requires bounded protocol 1 recipes')
    seen=set()
    for op in q['operations']:
        if (not isinstance(op,dict) or set(op)!={'op','id','case'} or op['op']!='subcurve_numeric_followup'
                or not isinstance(op['id'],str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id']) is None
                or op['id'] in seen or op.get('case') not in CASES):raise ValueError('invalid numeric followup recipe')
        seen.add(op['id'])

def run(op,host):
    validate_request(dict(protocol_version=1,operations=[op]))
    Rhino,System=host['Rhino'],host['System'];G=Rhino.Geometry;doc=Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand():raise ValueError('numeric followup requires idle execution')
    if list(doc.Objects):raise ValueError('numeric followup requires empty owned document')
    from join_probe import observe_command
    owned=[];ids=[];saved=doc.ModelAbsoluteTolerance;doc.ModelAbsoluteTolerance=1e-6
    view=doc.Views.ActiveView;vp=view.ActiveViewport;original=Rhino.DocObjects.ViewportInfo(vp)
    def keep(g):
        if g is None or not g.IsValid:raise ValueError('invalid numeric source')
        owned.append(g);return g
    def snapshot():
        rows=[]
        for obj in sorted(list(doc.Objects),key=lambda o:o.RuntimeSerialNumber):
            g=obj.Geometry;row=dict(source=ids.index(obj.Id) if obj.Id in ids else None,selected=bool(obj.IsSelected(False)))
            if isinstance(g,G.Curve):
                n=g.ToNurbsCurve()
                try:row.update(kind='curve',definition=host['_nurbs_curve_definition'](n),length=float(g.GetLength()),samples=[host['_xyz'](g.PointAt(g.Domain.ParameterAt(i/32.))) for i in range(33)])
                finally:n.Dispose()
            else:
                s=g.Faces[0].UnderlyingSurface() if isinstance(g,G.Brep) else g
                n=s.ToNurbsSurface()
                try:row.update(kind='surface',definition=host['_nurbs_surface_definition'](n))
                finally:n.Dispose()
            rows.append(row)
        return rows
    try:
        surface=keep(G.NurbsSurface.CreateFromCorners(G.Point3d(0.,0.,0.),G.Point3d(4.,0.,0.),G.Point3d(4.,6.,0.),G.Point3d(0.,6.,0.)))
        ids.append(doc.Objects.AddSurface(surface))
        case=op['case']
        if case=='confirm_nonuniform':
            curve=keep(G.PolylineCurve([G.Point3d(x,y,0.) for x,y in ((0.,0.),(6.,0.),(6.,1.),(0.,0.))]))
        elif 'closed' in case:
            curve=keep(G.PolylineCurve([G.Point3d(x,y,0.) for x,y in ((0.,0.),(4.,0.),(4.,6.),(0.,6.),(0.,0.))]))
        elif case=='confirm_quadratic':
            curve=keep(host['_nurbs_curve_from_definition'](dict(degree=2,knots=[2.,2.,2.,5.,5.,5.],control_points=[dict(point=p,weight=1.) for p in ((0.,0.,0.),(2.,6.,0.),(4.,6.,0.))])))
        else:curve=keep(G.LineCurve(G.Point3d(0.,0.,0.),G.Point3d(4.,6.,0.)))
        ids.append(doc.Objects.AddCurve(curve))
        ids.append(doc.Objects.AddCurve(keep(G.LineCurve(G.Point3d(10.,0.,0.),G.Point3d(10.5,0.,0.)))))
        command='SubCrv' if case.startswith('standalone') else ('ApplyCrv' if case.startswith('apply_') else 'CreateUVCrv')
        fraction=1./15. if case=='confirm_nonuniform' else (.8 if 'closed' in case else (.9 if case in ('confirm_tail','confirm_clamp','apply_clamp') else (.75 if case in ('confirm_reverse','confirm_negative','negative_backward','apply_reverse') else .25)))
        anchor=curve.Domain.ParameterAt(fraction)
        start_point=host['_xyz'](curve.PointAt(anchor))
        start_token=','.join(repr(x) for x in start_point)
        start='_SubCrv _SelID '+str(ids[1])+' '+start_token+' '
        length_token='1' if case=='confirm_nonuniform' else ('-2' if case.startswith('negative_') or case=='confirm_negative' else ('20' if case=='confirm_closed_full' else ('8' if 'closed' in case else ('20' if case=='confirm_unavailable' else '2'))))
        confirmation=[10.25,0.,0.]
        if case in ('confirm_reverse','negative_backward','apply_reverse'):confirmation=[0.2,0.3,0.]
        if case in ('confirm_closed_forward','confirm_closed_full','apply_closed'):confirmation=[3.,0.,0.]
        if case=='confirm_closed_backward':confirmation=[4.,3.,0.]
        if case=='confirm_nonuniform':confirmation=[6.,0.8,0.]
        if case in ('confirm_clamp','apply_clamp'):confirmation=[10.,10.,0.]
        if case=='confirm_same':confirmation=start_point
        if case=='point_control':tail='3,4.5,0 _Enter'
        elif case=='number_only':tail='2 _Enter _Enter'
        elif case=='zero':tail='0 _Enter'
        elif case=='confirm_coordinates':tail='2 3,4.5,0 _Enter _Enter'
        elif case=='confirm_unavailable':tail='20 3,4.5,0 _Enter'
        elif case=='replace_number':tail='2 1 3,4.5,0 _Enter _Enter'
        elif case=='confirm_empty':tail='2 5,5,0 _Enter _Enter'
        elif case in ('confirm_clamp','confirm_same'):tail=length_token+' '+','.join(repr(x) for x in confirmation)+' _Enter _Enter'
        elif case.startswith('negative_'):tail=length_token+' _Pause _Enter _Enter'
        elif case.startswith('confirm_'):tail=length_token+' _Pause _Enter _Enter'
        elif case.endswith('click'):tail='2 _Pause _Enter _Enter'
        else:tail='2 _SelID '+str(ids[2])+' _Enter _Enter'
        macro=start+tail
        if case=='empty_nested':macro='_SubCrv _Enter _Enter'
        if command=='CreateUVCrv':macro='_CreateUVCrv _SelID '+str(ids[0])+' '+macro
        elif command=='ApplyCrv':
            rectangle=keep(G.PolylineCurve([G.Point3d(x,y,0.) for x,y in ((0.,0.),(4.,0.),(4.,6.),(0.,6.),(0.,0.))]))
            ids.append(doc.Objects.AddCurve(rectangle))
            confirm=','.join(repr(x) for x in confirmation) if case=='apply_clamp' else '_Pause'
            macro='_ApplyCrv '+start+length_token+' '+confirm+' _SelID '+str(ids[3])+' _Enter _SelID '+str(ids[0])+' _Enter'
        else:macro='_SelID '+str(ids[1])+' _SubCrv _Copy=_Yes '+start_token+' '+tail
        if case.endswith('click') or case.startswith('negative_') or (case.startswith('apply_') and case!='apply_clamp') or (case.startswith('confirm_') and case not in ('confirm_coordinates','confirm_unavailable','confirm_empty','confirm_clamp','confirm_same')):
            if not vp.SetProjection(Rhino.Display.DefinedViewportProjection.Top,'Owned numeric reference',False):raise ValueError('numeric reference projection failed')
            vp.ZoomBoundingBox(G.BoundingBox(G.Point3d(-1.,-2.,-1.),G.Point3d(12.,8.,1.)))
            pixel=vp.WorldToClient(host['_point'](confirmation))
            screen=view.ClientToScreen(System.Drawing.Point(int(pixel.X),int(pixel.Y)))
            doc.Views.Redraw()
            host['_record_progress']('PICK @numeric-ref:%s %d %d' % (op['id'],screen.X,screen.Y))
        doc.ClearUndoRecords(True);before=snapshot();marker='Viboceros numeric followup '+str(System.Guid.NewGuid())
        Rhino.RhinoApp.WriteLine(marker)
        success,after,events=observe_command(Rhino.Commands.Command,command,lambda:Rhino.RhinoApp.RunScript(macro,True),snapshot,lambda:[],True)
        active=bool(Rhino.Commands.Command.InCommand())
        if active:Rhino.RhinoApp.RunScript('!',False)
        history=Rhino.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1]
        return dict(command=command,macro=macro,anchor=anchor,start_point=start_point,length_token=length_token,confirmation=confirmation,before=before,after=after,success=success,command_active=active,events=events,history=history),0
    finally:
        if Rhino.Commands.Command.InCommand():Rhino.RhinoApp.RunScript('!',False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id,True)
        doc.ModelAbsoluteTolerance=saved
        vp.SetViewProjection(original,False);original.Dispose()
        for g in reversed(owned):g.Dispose()

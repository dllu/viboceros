"""Owned public MarkFoci commands, without inspecting proprietary code."""
import math
import re

FAMILIES=('ellipse','ellipse_arc','circle','circle_arc','hyperbola','parabola','line','cubic','definition')

def finite(v):return type(v) in(int,float) and not(math.isnan(v)or math.isinf(v))

def validate(op):
    required={'op','id','family','artifact_path'}
    optional={'curve','draw_axis','preselect','domain','elevate','reverse','mixed','tilted','gauge','polycurve'}
    if (not isinstance(op,dict)or not required<=set(op)or set(op)-required-optional
        or op['op']!='mark_foci_command'or op['family']not in FAMILIES
        or not isinstance(op['id'],str)or re.match(r'^[A-Za-z0-9_-]{1,100}\Z',op['id'])is None
        or not isinstance(op['artifact_path'],str)or not op['artifact_path'].endswith('.3dm')):
        raise ValueError('invalid MarkFoci recipe')
    for k in ('draw_axis','preselect','reverse','mixed','tilted','polycurve'):
        if type(op.get(k,False))is not bool:raise ValueError('invalid MarkFoci flag')
    if type(op.get('elevate',2))is not int or not 2<=op.get('elevate',2)<=12:raise ValueError('invalid source elevation')
    if not finite(op.get('gauge',1.))or not 1e-200<=abs(op.get('gauge',1.))<=1e200:raise ValueError('invalid source weight scale')
    if 'domain'in op:
        d=op['domain']
        if not isinstance(d,list)or len(d)!=2 or not all(finite(x)for x in d)or not d[0]<d[1]:raise ValueError('invalid source domain')
    if op['family']=='definition':
        c=op.get('curve')
        if not isinstance(c,dict)or set(c)!={'degree','control_points','knots'}:raise ValueError('invalid conic definition')
        d=c['degree'];p=c['control_points'];k=c['knots']
        if type(d)is not int or not 1<=d<=12 or not isinstance(p,list)or not d+1<=len(p)<=128:raise ValueError('invalid conic controls')
        if not isinstance(k,list)or len(k)!=len(p)+d+1 or not all(finite(x)for x in k)or any(a>b for a,b in zip(k,k[1:])):raise ValueError('invalid conic knots')
        for cv in p:
            if not isinstance(cv,dict)or set(cv)!={'point','weight'}or not finite(cv['weight'])or cv['weight']==0.:raise ValueError('invalid conic weight')
            q=cv['point']
            if not isinstance(q,list)or len(q)!=3 or not all(finite(x)and abs(x)<=1e12 for x in q):raise ValueError('invalid conic point')
    elif 'curve'in op:raise ValueError('curve definition requires definition family')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('MarkFoci requires one iteration')
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    from join_probe import observe_command
    if R.Commands.Command.InCommand()or list(doc.Objects):raise ValueError('MarkFoci requires an owned empty idle document')
    owned=[];attrs=[];layers=[];groups=[];saved_layer=doc.Layers.CurrentLayerIndex
    def keep(c):
        if c is None or not c.IsValid:raise ValueError('invalid owned conic source')
        owned.append(c);return c
    def definition(points,weights):
        return keep(host['_nurbs_curve_from_definition'](dict(degree=2,knots=[0.]*3+[1.]*3,control_points=[dict(point=p,weight=w)for p,w in zip(points,weights)])))
    def record(g):
        if isinstance(g,G.Point):return dict(type='point',point=host['_xyz'](g.Location))
        if not isinstance(g,G.Curve):raise ValueError('unexpected MarkFoci output')
        v=host['_interchange_curve_record'](g)
        if isinstance(g,G.NurbsCurve):v['rational']=bool(g.IsRational)
        if isinstance(g,G.PolyCurve):v['segments']=[record(g.SegmentCurve(i))for i in range(g.SegmentCount)]
        return v
    def strings(o):
        s=o.GetUserStrings();return dict((str(k),str(s[k]))for k in s.AllKeys)if s else{}
    def snapshot():
        return [dict(id=str(o.Id),name=str(o.Attributes.Name)if o.Attributes.Name else None,layer=int(o.Attributes.LayerIndex),
            groups=list(int(i)for i in(o.Attributes.GetGroupList()or[])),color_source=str(o.Attributes.ColorSource),
            color=[int(o.Attributes.ObjectColor.R),int(o.Attributes.ObjectColor.G),int(o.Attributes.ObjectColor.B)],
            user_text=strings(o.Attributes),geometry_text=strings(o.Geometry),selected=bool(o.IsSelected(False)),geometry=record(o.Geometry))for o in doc.Objects]
    try:
        family=op['family']
        if family in('ellipse','ellipse_arc'):
            source=keep(G.Ellipse(G.Plane.WorldXY,5.,3.).ToNurbsCurve())
            if family=='ellipse_arc':source=keep(source.Trim(.2,1.7))
        elif family in('circle','circle_arc'):
            source=keep(G.ArcCurve(G.Arc(G.Circle(G.Plane.WorldXY,3.),G.Interval(.3,.3+(math.pi*2. if family=='circle'else 1.7)))))
        elif family=='hyperbola':source=definition([[3.75,-3.,0.],[2.4,0.,0.],[3.75,3.,0.]],[1.,1.25,1.])
        elif family=='parabola':source=definition([[-2.,1.,0.],[0.,-1.,0.],[2.,1.,0.]],[1.]*3)
        elif family=='line':source=keep(G.LineCurve(G.Point3d(0.,0.,0.),G.Point3d(3.,0.,0.)))
        elif family=='cubic':
            source=keep(host['_nurbs_curve_from_definition'](dict(degree=3,knots=[0.]*4+[1.]*4,control_points=[dict(point=p,weight=1.)for p in[[0.,0.,0.],[1.,3.,0.],[3.,-1.,0.],[5.,0.,0.]]])))
        else:source=keep(host['_nurbs_curve_from_definition'](op['curve']))
        if op.get('elevate',2)>2:
            source=keep(source.ToNurbsCurve())
            if not source.IncreaseDegree(op['elevate']):raise ValueError('source degree elevation failed')
        if op.get('gauge',1.)!=1.:
            source=keep(source.ToNurbsCurve());source.Points.MakeRational()
            for i in range(source.Points.Count):
                cv=source.Points[i]
                if not source.Points.SetPoint(i,cv.Location,cv.Weight*op['gauge']):raise ValueError('source gauge failed')
        if op.get('reverse')and not source.Reverse():raise ValueError('source reversal failed')
        if op.get('tilted'):
            plane=G.Plane(G.Point3d(3.,-4.,5.),G.Vector3d(1.,2.,3.))
            if not source.Transform(G.Transform.PlaneToPlane(G.Plane.WorldXY,plane)):raise ValueError('source placement failed')
        if op.get('polycurve'):
            pieces=source.Split(source.Domain.Mid)
            if pieces is None or len(pieces)!=2:raise ValueError('source polycurve split failed')
            owned.extend(pieces);source=G.PolyCurve();owned.append(source)
            for part in pieces:
                if not source.Append(part):raise ValueError('source polycurve append failed')
        if 'domain'in op:source.Domain=G.Interval(*op['domain'])
        source.SetUserString('GeometryCode','source')
        layer=R.DocObjects.Layer();layer.Name='FociSource_'+str(S.Guid.NewGuid())
        try:layer_index=doc.Layers.Add(layer)
        finally:layer.Dispose()
        layers.append(layer_index);sources=[source]
        if op.get('mixed'):
            witness=keep(G.Ellipse(G.Plane.WorldXY,7.,2.).ToNurbsCurve());witness.SetUserString('GeometryCode','source');sources.append(witness)
        ids=[]
        for i,c in enumerate(sources):
            a=R.DocObjects.ObjectAttributes();attrs.append(a);a.Name='FociSource'+str(i);a.LayerIndex=layer_index
            a.SetUserString('ObjectCode','source');a.ObjectColor=S.Drawing.Color.FromArgb(17,47,87);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
            key=doc.Objects.AddCurve(c,a)
            if key==S.Guid.Empty:raise ValueError('source insertion failed')
            ids.append(key)
        group=doc.Groups.Add('FociGroup_'+str(S.Guid.NewGuid()),S.Array[S.Guid](ids));groups.append(group)
        if op.get('preselect',False):
            for key in ids:doc.Objects.Select(key)
        path=op['artifact_path'];path='Z:'+path.replace('/','\\')if path.startswith('/')else path
        options=R.FileIO.FileWriteOptions();options.FileVersion=8;options.SuppressDialogBoxes=True;options.SuppressAllInput=True;options.WriteUserData=True;options.UpdateDocumentPath=False
        try:
            if not doc.Write3dmFile(path,options):raise ValueError('source artifact write failed')
        finally:options.Dispose()
        doc.ClearUndoRecords(True);before=snapshot();marker='Viboceros MarkFoci '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        macro='_MarkFoci '
        if not op.get('preselect',False):macro+='_DrawAxis=_%s '%('Yes'if op.get('draw_axis',False)else'No')+' '.join('_SelID '+str(key)for key in ids)+' _Enter'
        success,after,events=observe_command(R.Commands.Command,'MarkFoci',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[str(o.Id)for o in doc.Objects if o.IsSelected(False)],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1];changed=len(before)!=len(after);undo=redo=None
        if changed:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(recipe=op,source_ids=[str(k)for k in ids],before=before,after=after,changed=changed,undo=undo,redo=redo,success=success,events=events,history=history,macro=macro),0
    finally:
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        for o in list(doc.Objects):doc.Objects.Delete(o.Id,True)
        for i in groups:
            if i>=0 and not doc.Groups.IsDeleted(i):doc.Groups.Delete(i)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for i in layers:doc.Layers.Delete(i,True)
        for a in reversed(attrs):a.Dispose()
        for g in reversed(owned):g.Dispose()

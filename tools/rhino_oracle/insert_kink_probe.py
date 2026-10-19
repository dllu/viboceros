"""Owned public InsertKink command observations; no proprietary implementation."""
import math
import re

def validate(op):
    required={'op','id','family','stations','artifact_path'}
    optional={'cancel','preselect','direction','symmetrical','midpoints','numeric','domain','reverse','elevate','gauge','tilted','subcurve','face','v_fraction','refine'}
    if (not isinstance(op,dict)or not required<=set(op)or set(op)-required-optional or op['op']!='insert_kink_command'
        or not isinstance(op['id'],str)or re.match(r'^[A-Za-z0-9_-]{1,80}\Z',op['id'])is None
        or op['family']not in('cubic','circle','surface','line','polyline','ellipse','polycurve','rational','periodic','surface_rational','trimmed_rectangle','trimmed_hole','box')
        or not isinstance(op['artifact_path'],str)or not op['artifact_path'].endswith('.3dm')
        or not isinstance(op['stations'],list)or not 0<=len(op['stations'])<=16
        or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)or not 0<=x<=1 for x in op['stations'])):raise ValueError('invalid InsertKink recipe')
    for name in('cancel','preselect','symmetrical','midpoints','numeric','reverse','tilted'):
        if type(op.get(name,False))is not bool:raise ValueError('invalid InsertKink flag')
    if op.get('direction','U')not in('U','V','Both'):raise ValueError('invalid kink direction')
    if type(op.get('elevate',3))is not int or not 2<=op.get('elevate',3)<=12:raise ValueError('invalid elevation')
    for name in('gauge','v_fraction'):
        value=op.get(name,1. if name=='gauge'else .6)
        if type(value)not in(int,float)or math.isnan(value)or math.isinf(value)or (not 1e-200<=abs(value)<=1e200 if name=='gauge'else not 0<=value<=1):raise ValueError('invalid source scalar')
    if type(op.get('face',0))is not int or not 0<=op.get('face',0)<=5:raise ValueError('invalid source face')
    for name in('domain','subcurve'):
        if name in op:
            d=op[name]
            if not isinstance(d,list)or len(d)!=2 or any(type(x)not in(int,float)or math.isnan(x)or math.isinf(x)for x in d)or not d[0]<d[1]:raise ValueError('invalid source interval')
    if 'subcurve'in op and(op.get('preselect',True)or not 0<=op['subcurve'][0]<op['subcurve'][1]<=1):raise ValueError('invalid temporary range')
    if 'refine'in op and(type(op['refine'])not in(int,float)or not 0<op['refine']<1):raise ValueError('invalid refinement')

def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('InsertKink requires one iteration')
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    from join_probe import observe_command
    if R.Commands.Command.InCommand()or list(doc.Objects):raise ValueError('InsertKink requires an owned empty idle document')
    owned=[];attrs=[];layers=[];groups=[];saved_layer=doc.Layers.CurrentLayerIndex
    def keep(c):
        if c is None or not c.IsValid:raise ValueError('invalid kink source')
        owned.append(c);return c
    def record(g):
        if isinstance(g,G.Point):return dict(type='point',point=host['_xyz'](g.Location))
        if isinstance(g,G.Brep):
            return dict(type='brep',faces=int(g.Faces.Count),surfaces=[host['_nurbs_surface_definition'](f.UnderlyingSurface())for f in g.Faces],
                samples=[[host['_xyz'](f.PointAt(f.Domain(0).ParameterAt(u/8.),f.Domain(1).ParameterAt(v/8.)))for v in range(9)for u in range(9)]for f in g.Faces],
                topology=dict(vertices=[host['_xyz'](v.Location)for v in g.Vertices],edges=[[host['_xyz'](e.PointAtStart),host['_xyz'](e.PointAtEnd)]for e in g.Edges],loop_counts=[int(f.Loops.Count)for f in g.Faces],trim_counts=[int(l.Trims.Count)for f in g.Faces for l in f.Loops],reversed=[bool(f.OrientationIsReversed)for f in g.Faces]))
        if isinstance(g,G.Surface):return dict(type='surface',definition=host['_nurbs_surface_definition'](g))
        v=host['_interchange_curve_record'](g)
        if isinstance(g,G.NurbsCurve):v['rational']=bool(g.IsRational)
        if isinstance(g,G.PolylineCurve):
            n=g.ToNurbsCurve()
            try:
                v['definition']=host['_nurbs_curve_definition'](n)
                v['vertices']=[host['_xyz'](g.Point(i))for i in range(g.PointCount)]
                v['parameters']=v['definition']['knots'][1:-1]
            finally:n.Dispose()
        if isinstance(g,G.PolyCurve):v['segments']=[record(g.SegmentCurve(i))for i in range(g.SegmentCount)]
        return v
    def strings(o):
        s=o.GetUserStrings();return dict((str(k),str(s[k]))for k in s.AllKeys)if s else{}
    def snapshot():
        return [dict(id=str(o.Id),name=str(o.Attributes.Name)if o.Attributes.Name else None,layer=int(o.Attributes.LayerIndex),groups=list(int(i)for i in(o.Attributes.GetGroupList()or[])),user_text=strings(o.Attributes),geometry_text=strings(o.Geometry),selected=bool(o.IsSelected(False)),grips_on=bool(o.GripsOn),geometry=record(o.Geometry))for o in doc.Objects]
    try:
        family=op['family']
        if family=='surface_rational':
            source=keep(host['_nurbs_surface_from_definition'](dict(degree_u=2,degree_v=2,control_point_count_u=3,control_point_count_v=3,knots_u=[0.]*3+[1.]*3,knots_v=[0.]*3+[1.]*3,control_points=[dict(point=[4.*u,3.*v,float(u*v)],weight=1.+.2*(u+v))for v in range(3)for u in range(3)])))
        elif family=='box':source=keep(G.Brep.CreateFromBox(G.BoundingBox(G.Point3d(0.,0.,0.),G.Point3d(8.,6.,4.))))
        elif family in('surface','trimmed_rectangle','trimmed_hole'):
            source=keep(G.NurbsSurface.CreateFromCorners(G.Point3d(0.,0.,0.),G.Point3d(8.,0.,0.),G.Point3d(8.,6.,2.),G.Point3d(0.,6.,0.)))
            source.IncreaseDegreeU(3);source.IncreaseDegreeV(3)
            if family.startswith('trimmed'):
                base=keep(G.Brep.CreateFromSurface(source));du,dv=source.Domain(0),source.Domain(1)
                if family=='trimmed_hole':uv=keep(G.Circle(G.Plane(G.Point3d(du.Mid,dv.Mid,0.),G.Vector3d.ZAxis),.15*min(du.Length,dv.Length)).ToNurbsCurve())
                else:uv=keep(G.PolylineCurve(S.Array[G.Point3d]([G.Point3d(du.ParameterAt(u),dv.ParameterAt(v),0.)for u,v in[(.1,.1),(.9,.1),(.9,.9),(.1,.9),(.1,.1)]])))
                edge=keep(source.Pushup(uv,1e-8));split=keep(base.Faces[0].Split(S.Array[G.Curve]([edge]),1e-6));candidates=[keep(f.DuplicateFace(False))for f in split.Faces]
                def area(b):
                    mp=G.AreaMassProperties.Compute(b)
                    try:return mp.Area
                    finally:mp.Dispose()
                source=(max if family=='trimmed_hole'else min)(candidates,key=area)
        elif family=='circle':source=keep(G.ArcCurve(G.Circle(G.Plane.WorldXY,3.)))
        elif family=='ellipse':source=keep(G.Ellipse(G.Plane.WorldXY,5.,3.).ToNurbsCurve())
        elif family=='line':source=keep(G.LineCurve(G.Point3d(0.,0.,0.),G.Point3d(6.,0.,0.)))
        elif family=='polyline':source=keep(G.PolylineCurve(S.Array[G.Point3d]([G.Point3d(0.,0.,0.),G.Point3d(3.,0.,0.),G.Point3d(3.,4.,0.),G.Point3d(6.,4.,0.)])))
        elif family=='polycurve':
            source=G.PolyCurve();owned.append(source)
            for c in [G.LineCurve(G.Point3d(0.,0.,0.),G.Point3d(3.,0.,0.)),G.ArcCurve(G.Arc(G.Point3d(3.,0.,0.),G.Point3d(5.,2.,0.),G.Point3d(3.,4.,0.)))]:
                keep(c)
                if not source.Append(c):raise ValueError('source append failed')
        else:
            points=[[0.,0.,0.],[1.,3.,0.],[3.,-1.,0.],[5.,0.,0.]]
            if family=='periodic':
                source=keep(G.NurbsCurve.Create(True,3,S.Array[G.Point3d]([G.Point3d(*p)for p in points])))
            else:
                source=keep(host['_nurbs_curve_from_definition'](dict(degree=3,knots=[0.]*4+[1.]*4,control_points=[dict(point=p,weight=1.+i%3 if family=='rational'else 1.)for i,p in enumerate(points)])))
                if family=='cubic':source.Points.MakeNonRational()
        if op.get('elevate',3)>3:
            source=keep(source.ToNurbsCurve())
            if not source.IncreaseDegree(op['elevate']):raise ValueError('elevation failed')
        if 'refine'in op:
            source=keep(source.ToNurbsCurve())
            if not source.Knots.InsertKnot(source.Domain.ParameterAt(op['refine']),source.Degree):raise ValueError('refinement failed')
        if op.get('gauge',1.)!=1.:
            source=keep(source.ToNurbsCurve());source.Points.MakeRational()
            for i in range(source.Points.Count):
                cv=source.Points[i]
                if not source.Points.SetPoint(i,cv.Location,cv.Weight*op['gauge']):raise ValueError('source gauge failed')
        if op.get('reverse')and not source.Reverse():raise ValueError('source reversal failed')
        if op.get('tilted')and not source.Transform(G.Transform.PlaneToPlane(G.Plane.WorldXY,G.Plane(G.Point3d(3.,-4.,5.),G.Vector3d(1.,2.,3.)))):raise ValueError('source transform failed')
        if 'domain'in op:source.Domain=G.Interval(*op['domain'])
        source.SetUserString('GeometryCode','source')
        layer=R.DocObjects.Layer();layer.Name='KinkSource_'+str(S.Guid.NewGuid())
        try:layer_index=doc.Layers.Add(layer)
        finally:layer.Dispose()
        layers.append(layer_index)
        a=R.DocObjects.ObjectAttributes();attrs.append(a);a.Name='KinkSource';a.SetUserString('ObjectCode','source');a.LayerIndex=layer_index
        a.ObjectColor=S.Drawing.Color.FromArgb(17,47,87);a.ColorSource=R.DocObjects.ObjectColorSource.ColorFromObject
        key=doc.Objects.Add(source,a)
        if key==S.Guid.Empty:raise ValueError('source insertion failed')
        group=doc.Groups.Add('KinkGroup_'+str(S.Guid.NewGuid()),S.Array[S.Guid]([key]));groups.append(group)
        if op.get('preselect',True):
            if 'face'in op:
                if not doc.Objects.FindId(key).SelectSubObject(G.ComponentIndex(G.ComponentIndexType.BrepFace,op['face']),True,True,False):raise ValueError('face selection failed')
            else:doc.Objects.Select(key)
        path=op['artifact_path'];path='Z:'+path.replace('/','\\')if path.startswith('/')else path
        options=R.FileIO.FileWriteOptions();options.FileVersion=8;options.SuppressDialogBoxes=True;options.SuppressAllInput=True;options.WriteUserData=True;options.UpdateDocumentPath=False
        try:
            if not doc.Write3dmFile(path,options):raise ValueError('source write failed')
        finally:options.Dispose()
        surface_family=family in('surface','surface_rational','trimmed_rectangle','trimmed_hole','box')
        target=source.Faces[op.get('face',0)]if isinstance(source,G.Brep)else source
        picks=[]
        for f in op['stations']:
            p=target.PointAt(target.Domain(0).ParameterAt(f),target.Domain(1).ParameterAt(op.get('v_fraction',.6)))if surface_family else source.PointAt(source.Domain.ParameterAt(f))
            picks.append(host['_xyz'](p))
        macro='_InsertKink '
        if 'subcurve'in op:
            ends=[source.PointAt(source.Domain.ParameterAt(f))for f in op['subcurve']]
            macro+='_SubCrv _SelID '+str(key)+' '+' '.join('w'+','.join('%.17g'%x for x in host['_xyz'](p))for p in ends)+' '
        elif not op.get('preselect',True):macro+='_SelID '+str(key)+' '
        if surface_family:macro+='_Direction=_'+op.get('direction','U')+' _Symmetrical=_'+('Yes'if op.get('symmetrical')else'No')+' _Midpoints=_'+('Yes'if op.get('midpoints')else'No')+' '
        macro+=(' '.join('%.17g'%source.Domain.ParameterAt(f)for f in op['stations'])if op.get('numeric')else' '.join('w'+','.join('%.17g'%x for x in p)for p in picks))
        macro+=' !'if op.get('cancel')else' _Enter'
        doc.ClearUndoRecords(True);before=snapshot();marker='Viboceros InsertKink '+str(S.Guid.NewGuid());R.RhinoApp.WriteLine(marker)
        success,after,events=observe_command(R.Commands.Command,'InsertKink',lambda:R.RhinoApp.RunScript(macro,True),snapshot,lambda:[str(o.Id)for o in doc.Objects if o.IsSelected(False)],True)
        history=R.RhinoApp.CommandHistoryWindowText.split(marker,1)[-1];undo=redo=None
        if [(o['id'],o['geometry'])for o in before]!=[(o['id'],o['geometry'])for o in after]:
            R.RhinoApp.RunScript('_Undo',False);undo=snapshot();R.RhinoApp.RunScript('_Redo',False);redo=snapshot()
        return dict(recipe=op,source_id=str(key),picks=picks,macro=macro,success=success,before=before,after=after,undo=undo,redo=redo,history=history,events=events),0
    finally:
        if R.Commands.Command.InCommand():R.RhinoApp.RunScript('!',False)
        for obj in list(doc.Objects):obj.GripsOn=False;doc.Objects.Delete(obj.Id,True)
        for group in groups:
            if group>=0 and not doc.Groups.IsDeleted(group):doc.Groups.Delete(group)
        doc.Layers.SetCurrentLayerIndex(saved_layer,True)
        for layer in layers:doc.Layers.Delete(layer,True)
        for a in reversed(attrs):a.Dispose()
        for g in reversed(owned):g.Dispose()

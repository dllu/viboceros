"""Read-only public OffsetOnSurface SDK observations on fixed source scenes."""
import re
if __package__:from .surface_offset_command_probe import CURVED_CASES,GENERAL_CASES,CORNER_CASES,curved_source,general_source,corner_source
else:from surface_offset_command_probe import CURVED_CASES,GENERAL_CASES,CORNER_CASES,curved_source,general_source,corner_source
STANDARD_CASES=('plane_line','plane_curve','cylinder_axis','cylinder_ring','sphere_latitude','warped_iso')
CASES=STANDARD_CASES+('plane_circle','plane_seam','plane_bound')+tuple(CURVED_CASES)+tuple(GENERAL_CASES)+tuple(CORNER_CASES)
def validate(op):
    if not isinstance(op,dict)or set(op)-{'samples'}!={'op','id','case','distance'}or op['op']!='surface_curve_offset':raise ValueError('invalid surface curve offset recipe')
    if not isinstance(op['id'],str)or re.match(r'\A[A-Za-z0-9_-]{1,80}\Z',op['id'])is None or op['case']not in CASES:raise ValueError('invalid surface curve offset case')
    if type(op['distance'])not in(int,float)or not 0<abs(op['distance'])<=2:raise ValueError('invalid surface offset distance')
    if 'samples'in op and(not isinstance(op['samples'],list)or not 1<=len(op['samples'])<=64 or any(not isinstance(p,list)or len(p)!=3 or any(type(x)not in(int,float)or not abs(x)<=1000 for x in p)for p in op['samples'])):raise ValueError('invalid bounded offset query points')
def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='surface_curve_offset',id=case+'_'+str(i),case=case,distance=d)for case in STANDARD_CASES for i,d in enumerate([.25,-.25,1.,-1.])])
def clipping_request():
    q=request();q['operations']=[dict(op='surface_curve_offset',id=name,case=case,distance=distance)for name,case,distance in [('four_arcs','plane_circle',-.25),('tangent_circle','plane_circle',-.2),('seam_arc','plane_seam',-.25),('line_ends','plane_bound',.25),('empty_circle','plane_circle',-1.)]]
    return q
def extension_request():
    q=request();q['operations']=[dict(op='surface_curve_offset',id=case,case=case,distance=d)for case,d in CURVED_CASES.items()];return q
def corner_request():
    q=request();q['operations']=[dict(op='surface_curve_offset',id=case,case=case,distance=d)for case,d in CORNER_CASES.items()];return q
def general_extension_request():
    q=request();q['operations']=[dict(op='surface_curve_offset',id=case,case=case,distance=d)for case,d in GENERAL_CASES.items()];return q
def general_verify_request():
    q=request();q['operations']=[dict(op='surface_curve_offset',id='general_iso_verify',case='general_iso',distance=-.05,
        samples=[[1.005111040029211,.9532700196247835,.2829652695384036]])];return q
def validate_request(q):
    if type(q.get('iterations',1))is not int or q.get('iterations',1)!=1 or not isinstance(q.get('operations'),list)or not 1<=len(q['operations'])<=64:raise ValueError('bounded one-iteration surface offset SDK request required')
    names=[]
    for op in q['operations']:validate(op);names.append(op['id'])
    if len(set(names))!=len(names):raise ValueError('unique offset case names required')
def run(op,host,iterations,tolerance):
    validate(op)
    if iterations!=1:raise ValueError('one observation per source')
    G=host['Rhino'].Geometry;owned=[]
    def keep(g):
        if g is None or not g.IsValid:raise ValueError('invalid owned offset source')
        owned.append(g);return g
    try:
        case=op['case']
        if case in CORNER_CASES:
            surface=keep(G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p)for p in[[-10,-10,0],[10,-10,0],[10,10,0],[-10,10,0]]]))
            source=keep(corner_source(G,case))
            if case=='corner_tilted':
                rotation=G.Transform.Rotation(.6,G.Vector3d.XAxis,G.Point3d.Origin);source.Transform(rotation);surface.Transform(rotation)
        elif case in GENERAL_CASES:
            surface,source=general_source(host,case);keep(surface);keep(source)
        elif case in CURVED_CASES:
            surface,source=curved_source(G,case);keep(surface);keep(source)
        elif case in('plane_circle','plane_seam','plane_bound'):
            corners=[[0,0,0],[1,0,0],[1,1,0],[0,1,0]]if case=='plane_bound'else[[-1,-1,0],[1,-1,0],[1,1,0],[-1,1,0]]
            surface=keep(G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p)for p in corners]))
            source=keep(G.LineCurve(G.Point3d(0,.2,0),G.Point3d(1,.8,0)))if case=='plane_bound'else keep(G.Circle(G.Point3d(0,.3 if case=='plane_seam'else 0,0),.6 if case=='plane_seam'else .8).ToNurbsCurve())
        elif case.startswith('plane'):
            surface=keep(G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p)for p in[[-10,-10,0],[10,-10,0],[10,10,0],[-10,10,0]]]))
            source=keep(G.LineCurve(G.Point3d(-4,0,0),G.Point3d(4,0,0)))if case=='plane_line'else keep(G.Curve.CreateControlPointCurve([G.Point3d(*p)for p in[[-4,0,0],[-2,3,0],[2,-3,0],[4,0,0]]],3))
        elif case.startswith('cylinder'):
            surface=keep(G.Cylinder(G.Circle(G.Plane.WorldXY,3.),6.).ToNurbsSurface())
            source=keep(surface.IsoCurve(1,.6)if case=='cylinder_axis'else surface.IsoCurve(0,3.))
        elif case=='sphere_latitude':
            surface=keep(G.Sphere(G.Point3d.Origin,4.).ToNurbsSurface());source=keep(surface.IsoCurve(0,.2))
        else:
            surface=keep(G.NurbsSurface.CreateFromCorners(*[G.Point3d(*p)for p in[[-5,-5,0],[5,-5,0],[5,5,3],[-5,5,-1]]]))
            source=keep(surface.IsoCurve(0,.5))
        face=keep(surface.ToBrep()).Faces[0]
        before=[int(source.DataCRC(0)),int(surface.DataCRC(0))]
        curves=list(source.OffsetOnSurface(face,float(op['distance']),1e-8)or[])
        owned.extend(curves)
        outputs=[]
        def definition(curve):
            if curve is None:return None
            n=curve.ToNurbsCurve()
            try:return host['_nurbs_curve_definition'](n)
            finally:n.Dispose()
        extensions=[];extended_source=None;source_first=[];further=[]
        if case in CURVED_CASES or case in GENERAL_CASES:
            extended=source.ExtendOnSurface(G.CurveEnd.Both,surface)
            if extended is not None:
                owned.append(extended);extended_source=definition(extended)
                further=list(extended.OffsetOnSurface(face,float(op['distance']),1e-8)or[]);owned.extend(further)
                source_first=[definition(c)for c in further]
        for curve in curves:
            if not curve.IsValid:raise ValueError('invalid offset result')
            outputs.append(dict(definition=host['_nurbs_curve_definition'](curve),samples=[host['_xyz'](curve.PointAt(curve.Domain.ParameterAt(i/64.)))for i in range(65)],length=float(curve.GetLength())))
            if case in CURVED_CASES or case in GENERAL_CASES:
                extended=curve.ExtendOnSurface(G.CurveEnd.Both,surface)
                if extended is not None:owned.append(extended)
                extensions.append(definition(extended))
        if before!=[int(source.DataCRC(0)),int(surface.DataCRC(0))]:raise ValueError('SDK mutated offset inputs')
        distances=[];source_first_distances=[]
        for p in op.get('samples',[]):
            point=G.Point3d(*p);found=[]
            for curve in curves:
                ok,t=curve.ClosestPoint(point)
                if not ok:raise ValueError('SDK closest point failed')
                found.append(float(point.DistanceTo(curve.PointAt(t))))
            distances.append(min(found)if found else None)
            found=[]
            for curve in further:
                ok,t=curve.ClosestPoint(point)
                if not ok:raise ValueError('SDK source-first closest point failed')
                found.append(float(point.DistanceTo(curve.PointAt(t))))
            source_first_distances.append(min(found)if found else None)
        return dict(surface=host['_nurbs_surface_definition'](surface),curve=host['_nurbs_curve_definition'](source),outputs=outputs,source_unchanged=True,sample_distances=distances,source_first_sample_distances=source_first_distances,extended_offsets=extensions,extended_source=extended_source,offset_extended_source=source_first),0
    finally:
        for g in reversed(owned):g.Dispose()

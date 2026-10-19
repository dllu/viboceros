"""Public periodic, singular and dense surface sources for SoftEditSrf."""
import math
try:
    from . import soft_edit_surface_probe as base
except (ImportError,ValueError):
    import soft_edit_surface_probe as base

EXTRA=('shape','domains','reverse_u','reverse_v','transpose','gauge','degree_u','degree_v')


def mapped(op):
    return dict((k,v)for k,v in dict(op,op='soft_edit_surface_command',family='surface_rational').items()if k not in EXTRA)


def validate(op):
    if not isinstance(op,dict)or op.get('op')!='soft_edit_surface_shapes':raise ValueError('invalid surface shape recipe')
    base.validate(mapped(op))
    if op.get('shape')not in('flat','wave','rational','periodic','cylinder','sphere','torus'):raise ValueError('invalid surface family')
    for name in('reverse_u','reverse_v','transpose'):
        if type(op.get(name,False))is not bool:raise ValueError('invalid chart flag')
    for name in('degree_u','degree_v'):
        if type(op.get(name,3))is not int or not 1<=op.get(name,3)<=7:raise ValueError('invalid degree')
    g=op.get('gauge',1.)
    if type(g)not in(int,float)or not 1e-100<=abs(g)<=1e100:raise ValueError('invalid weight gauge')
    if 'domains'in op:
        ds=op['domains']
        if not isinstance(ds,list)or len(ds)!=2:raise ValueError('invalid domains')
        for d in ds:
            if not isinstance(d,list)or len(d)!=2 or any(type(x)not in(int,float)or abs(x)>1e100 for x in d)or not d[0]<d[1]:raise ValueError('invalid domain')


def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System'];G=R.Geometry;factory=host['_nurbs_surface_from_definition'];state={}
    def source(definition):
        shape=op['shape']
        if shape in('flat','wave'):
            nu,nv=(4,4)if shape=='flat'else(7,6)
            definition=dict(degree_u=3,degree_v=3,control_point_count_u=nu,control_point_count_v=nv,
                knots_u=[0.]*4+[float(i)for i in range(1,nu-3)]+[float(nu-3)]*4,
                knots_v=[0.]*4+[float(i)for i in range(1,nv-3)]+[float(nv-3)]*4,
                control_points=[dict(point=[float(u),1.2*v,0. if shape=='flat'else .4*math.sin(u)*math.cos(v)+.1*u*v],weight=1.)for v in range(nv)for u in range(nu)])
            n=factory(definition)
        elif shape=='rational':n=factory(definition)
        elif shape=='cylinder':n=G.Cylinder(G.Circle(G.Plane.WorldXY,3.),6.).ToNurbsSurface()
        elif shape=='sphere':n=G.Sphere(G.Point3d.Origin,3.).ToNurbsSurface()
        elif shape=='torus':n=G.Torus(G.Plane.WorldXY,5.,1.5).ToNurbsSurface()
        else:
            c=G.NurbsCurve.Create(True,3,S.Array[G.Point3d]([G.Point3d(*p)for p in [[0.,0.,0.],[1.,3.,0.],[3.,-1.,0.],[5.,0.,0.]]]))
            try:
                extruded=G.Surface.CreateExtrusion(c,G.Vector3d(0.,0.,6.))
                try:n=extruded.ToNurbsSurface()
                finally:extruded.Dispose()
            finally:c.Dispose()
        try:
            for direction,name in((0,'degree_u'),(1,'degree_v')):
                if name in op and op[name]>n.Degree(direction):
                    if not(n.IncreaseDegreeU(op[name])if direction==0 else n.IncreaseDegreeV(op[name])):raise ValueError('degree elevation failed')
            if op.get('reverse_u'):n.Reverse(0,True)
            if op.get('reverse_v'):n.Reverse(1,True)
            d=host['_nurbs_surface_definition'](n);nu,nv=d['control_count'];pu,pv=d['degree'];ku,kv=d['knots_u'],d['knots_v'];cp=d['control_points']
            if op.get('transpose'):
                cp=[cp[v*nu+u]for u in range(nu)for v in range(nv)];nu,nv=nv,nu;pu,pv=pv,pu;ku,kv=kv,ku
            definition=dict(degree_u=pu,degree_v=pv,control_point_count_u=nu,control_point_count_v=nv,
                knots_u=ku,knots_v=kv,control_points=[dict(p,weight=p['weight']*op.get('gauge',1.))for p in cp])
            if 'domains'in op:definition.update(domain_u=op['domains'][0],domain_v=op['domains'][1])
            return factory(definition)
        finally:n.Dispose()
    local=dict(host,_nurbs_surface_from_definition=source)
    value,elapsed=base.run(mapped(op),local,iterations,tolerance);value['recipe']=op
    return value,elapsed

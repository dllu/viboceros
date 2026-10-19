"""Owned RemoveKnot commands on exact and genuinely deforming source nets."""
import math
try:
    from . import insert_kink_probe
except (ImportError, ValueError):
    import insert_kink_probe

def plain_recipe(op):
    plain=dict(op)
    for key in('actions','seed','deform','periodic_surface_axis'):plain.pop(key,None)
    plain.update(op='insert_kink_command',stations=[])
    return plain

def validate(op):
    if not isinstance(op,dict)or op.get('op')!='remove_knot_command':raise ValueError('invalid RemoveKnot operation')
    insert_kink_probe.validate(plain_recipe(op))
    if op.get('direction','U')not in('U','V'):raise ValueError('removal direction requires U or V')
    for flag in('seed','deform'):
        if type(op.get(flag,False))is not bool:raise ValueError('invalid removal source flag')
    if op.get('deform')and not op.get('seed'):raise ValueError('deformation requires seeded source')
    if 'periodic_surface_axis'in op and(op['periodic_surface_axis']not in('U','V')or op['family']!='surface'):raise ValueError('invalid periodic source')
    actions=op.get('actions')
    if not isinstance(actions,list)or len(actions)>32:raise ValueError('invalid removal actions')
    for action in actions:
        if isinstance(action,str):
            if action not in('Undo','Toggle','Direction=U','Direction=V'):raise ValueError('invalid removal option')
        elif isinstance(action,dict)and set(action)=={'point'}:
            values=action['point']if isinstance(action['point'],list)else[action['point']]
            if len(values)not in(1,2)or any(type(x)not in(int,float)or not 0<=x<=1 for x in values):raise ValueError('invalid removal point')
        elif isinstance(action,dict)and set(action)=={'parameter'}:
            x=action['parameter']
            if type(x)not in(int,float)or math.isnan(x)or math.isinf(x)or abs(x)>1e6:raise ValueError('invalid removal parameter')
        else:raise ValueError('invalid removal action')

class Forward:
    def __init__(self,target):self.target=target
    def __getattr__(self,name):return getattr(self.target,name)
    def __iter__(self):return iter(self.target)

def run(op,host,iterations,tolerance):
    validate(op)
    import join_probe
    R=host['Rhino'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    app=Forward(R.RhinoApp);rhino=Forward(R);rhino.RhinoApp=app
    doc_type=Forward(R.RhinoDoc);shadow_doc=Forward(doc);objects=Forward(doc.Objects);shadow_doc.Objects=objects;doc_type.ActiveDoc=shadow_doc;rhino.RhinoDoc=doc_type
    state=dict(macro=None,picks=[],tokens=[]);owned=[]
    def keep(g):
        if g is None:raise ValueError('missing prepared removal source')
        owned.append(g);return g
    def prepare(source):
        if op['family']=='box':return source
        if 'periodic_surface_axis'in op:
            points=host['System'].Array[G.Point3d]([G.Point3d(0.,0.,0.),G.Point3d(1.,3.,0.),G.Point3d(3.,-1.,0.),G.Point3d(5.,0.,0.)])
            rail=keep(G.NurbsCurve.Create(True,3,points));extrusion=keep(G.Surface.CreateExtrusion(rail,G.Vector3d(0.,0.,5.)));source=keep(extrusion.ToNurbsSurface())
            axis=0 if op['periodic_surface_axis']=='U'else 1
            if not source.IsPeriodic(axis):source=keep(keep(source.Transpose()).ToNurbsSurface())
            if not source.IsPeriodic(axis):raise ValueError('periodic source axis mismatch')
        if not op.get('seed'):return source
        if isinstance(source,G.Curve):
            curve=keep(source.ToNurbsCurve())
            for f in(.2,.5,.8):
                if not curve.Knots.InsertKnot(curve.Domain.ParameterAt(f),1):raise ValueError('source curve seeding failed')
            if op.get('deform'):
                unique=curve.Points.Count-curve.Degree if curve.IsPeriodic else curve.Points.Count;i=unique//2;cv=curve.Points[i];point=G.Point3d(cv.Location.X,cv.Location.Y+.75,cv.Location.Z+.4)
                for index in range(i,curve.Points.Count,unique):
                    if not curve.Points.SetPoint(index,point,cv.Weight):raise ValueError('source control edit failed')
            result=curve
        else:
            brep=keep(source.DuplicateBrep())if isinstance(source,G.Brep)else None
            surface=keep((brep.Faces[0].UnderlyingSurface()if brep else source).ToNurbsSurface())
            for knots,axis in((surface.KnotsU,0),(surface.KnotsV,1)):
                for f in(.2,.5,.8):
                    if not knots.InsertKnot(surface.Domain(axis).ParameterAt(f),1):raise ValueError('source surface seeding failed')
            if op.get('deform'):
                i,j=surface.Points.CountU//2,surface.Points.CountV//2;cv=surface.Points.GetControlPoint(i,j)
                if not surface.Points.SetControlPoint(i,j,G.ControlPoint(G.Point3d(cv.Location.X+.3,cv.Location.Y-.2,cv.Location.Z+.75),cv.Weight)):raise ValueError('source surface edit failed')
            if brep:
                index=brep.AddSurface(surface)
                if index<0 or not brep.Faces[0].ChangeSurface(index):raise ValueError('source surface swap failed')
                if op.get('deform')and not brep.Faces[0].RebuildEdges(1e-8,True,True):raise ValueError('source edge rebuilding failed')
                brep.Compact();result=brep
            else:result=surface
        result.SetUserString('GeometryCode','source')
        if not result.IsValid:raise ValueError('invalid prepared removal source')
        return result
    objects.Add=lambda source,attrs:doc.Objects.Add(prepare(source),attrs)
    def execute(macro,echo):
        if not macro.startswith('_InsertKink '):return R.RhinoApp.RunScript(macro,echo)
        live=list(doc.Objects)
        if len(live)!=1:raise ValueError('expected one owned removal source')
        source=live[0].Geometry;key=live[0].Id;surface_family=op['family']in('surface','surface_rational','trimmed_rectangle','trimmed_hole','box')
        target=source.Faces[op.get('face',0)]if isinstance(source,G.Brep)else source
        def word(point):return 'w'+','.join('%.17g'%x for x in host['_xyz'](point))
        prefix='_RemoveKnot '
        if 'subcurve'in op:prefix+='_SubCrv _SelID '+str(key)+' '+' '.join(word(source.PointAt(source.Domain.ParameterAt(f)))for f in op['subcurve'])+' '
        elif not op.get('preselect',True):prefix+='_SelID '+str(key)+' '
        tokens=['_Direction=_'+op.get('direction','U')]if surface_family else[]
        for action in op['actions']:
            if isinstance(action,str):tokens.append('_'+action.replace('=','=_'))
            elif 'parameter'in action:tokens.append('%.17g'%action['parameter'])
            else:
                v=action['point'];uv=v if isinstance(v,list)else[v,op.get('v_fraction',.6)]
                point=target.PointAt(target.Domain(0).ParameterAt(uv[0]),target.Domain(1).ParameterAt(uv[1]))if surface_family else source.PointAt(source.Domain.ParameterAt(uv[0]))
                state['picks'].append(host['_xyz'](point));tokens.append(word(point))
        tokens.append('!'if op.get('cancel')else'_Enter');state['tokens']=tokens;state['macro']=prefix+' '.join(tokens)
        return R.RhinoApp.RunScript(state['macro'],echo)
    app.RunScript=execute;original=join_probe.observe_command
    def observe(event_source,command,*args,**kw):return original(event_source,'RemoveKnot'if command=='InsertKink'else command,*args,**kw)
    local=dict(host);local['Rhino']=rhino;join_probe.observe_command=observe
    try:
        value,elapsed=insert_kink_probe.run(plain_recipe(op),local,iterations,tolerance)
        value.update(recipe=op,macro=state['macro'],picks=state['picks'],tokens=state['tokens']);return value,elapsed
    finally:
        join_probe.observe_command=original
        for g in reversed(owned):g.Dispose()

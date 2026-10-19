"""Owned Reparameterize commands and public BrepFace.SetDomain controls."""
import math
import re
if __package__:
    from .surface_fillet_probe import validate_surface
else:
    from surface_fillet_probe import validate_surface

FAMILIES = ('annulus', 'reversed_rectangle', 'cylinder', 'sphere') + tuple('box_'+str(i) for i in range(6))


def validate(op):
    if (not isinstance(op, dict) or set(op)-{'surface'} != {'op', 'id', 'family', 'domains', 'artifact_path'}
            or op['op'] != 'reparameterize_face_command' or op['family'] not in FAMILIES+('nurbs',)
            or not isinstance(op['id'], str) or re.match(r'^[A-Za-z0-9_-]{1,100}\Z', op['id']) is None
            or not isinstance(op['artifact_path'], str) or not op['artifact_path'].endswith('.3dm')):
        raise ValueError('invalid face reparameterization recipe')
    if (op['family'] == 'nurbs') != ('surface' in op):
        raise ValueError('NURBS cases require one explicit surface definition')
    if 'surface' in op: validate_surface(op['surface'])
    domains = op['domains']
    if domains == 'Automatic': return
    if (not isinstance(domains, list) or len(domains) != 2
            or any(not isinstance(d, list) or len(d) != 2 or any(type(x) not in (int, float) or math.isnan(x) or math.isinf(x) for x in d) or d[0] >= d[1] for d in domains)):
        raise ValueError('face domains must be two increasing finite intervals')


def request(artifact_root):
    operations = []
    for family in FAMILIES:
        for name, domains in [('explicit', [[-2., 8.], [10., 30.]]), ('anisotropic', [[2e-8, 7e-8], [-8000., -3000.]]), ('automatic', 'Automatic')]:
            key = family+'_'+name
            operations.append(dict(op='reparameterize_face_command', id=key, family=family, domains=domains,
                                   artifact_path=artifact_root.rstrip('/')+'/'+key+'.3dm'))
    return dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-8, relative=1e-12, angular=1e-8), operations=operations)


def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1: raise ValueError('face command probes require one iteration')
    import Rhino
    import System
    from join_probe import observe_command
    G = Rhino.Geometry
    doc = Rhino.RhinoDoc.ActiveDoc
    if Rhino.Commands.Command.InCommand() or list(doc.Objects):
        raise ValueError('face reparameterization needs an empty owned idle document')
    family = op['family']; face_index = int(family[4:]) if family.startswith('box_') else 0
    source = control = None
    owned = []
    saved_tolerance = doc.ModelAbsoluteTolerance
    epsilon = tolerance['absolute']
    doc.ModelAbsoluteTolerance = epsilon
    def keep(g):
        if g is None or not g.IsValid: raise ValueError('invalid face reparameterization source')
        owned.append(g);return g
    def record(b):
        return dict(valid=bool(b.IsValid), geometry=host['_interchange_brep_record'](b))
    try:
        if family == 'nurbs':
            surface = keep(host['_nurbs_surface_from_definition'](op['surface']))
            source = keep(surface.ToBrep())
        elif family.startswith('box_'):
            source = keep(G.BoundingBox(G.Point3d(0., 0., 0.), G.Point3d(4., 6., 2.)).ToBrep())
        elif family == 'annulus':
            curves = [keep(G.Circle(G.Plane.WorldXY, r).ToNurbsCurve()) for r in (5., 2.)]
            parts = list(G.Brep.CreatePlanarBreps(curves, epsilon) or [])
            if len(parts) != 1: raise ValueError('annulus source failed')
            source = keep(parts[0])
        elif family == 'reversed_rectangle':
            outer = keep(G.PlaneSurface(G.Plane.WorldXY, G.Interval(-5., 5.), G.Interval(-4., 4.)))
            source = keep(outer.ToBrep())
            # Shrinking the natural domain is a genuine rectangular trim on
            # the full support when performed through the public face API.
            trimmed = list(source.Trim(G.Plane(G.Point3d(-3.,0.,0.), G.Vector3d(-1.,0.,0.)), epsilon) or [])
            if len(trimmed) != 1: raise ValueError('rectangular source trim failed')
            source = keep(trimmed[0]);source.Flip()
        elif family == 'cylinder':
            surface = keep(G.Cylinder(G.Circle(G.Plane.WorldXY, 2.), 3.).ToNurbsSurface())
            source = keep(surface.ToBrep())
        else:
            # Share the NURBS input itself. An analytic sphere's angle
            # parameters differ from its rational NURBS form, so equal sample
            # indices on those representations are not corresponding points.
            surface = keep(G.Sphere(G.Point3d.Origin, 2.).ToNurbsSurface())
            source = keep(surface.ToBrep())
        source.SetUserString('GeometryCode', 'retained')
        attrs = Rhino.DocObjects.ObjectAttributes();owned.append(attrs)
        attrs.Name = 'UV source';attrs.SetUserString('ObjectCode', 'retained')
        path = op['artifact_path']
        if path.startswith('/'): path = 'Z:'+path.replace('/', '\\')
        model = Rhino.FileIO.File3dm();owned.append(model)
        model.Settings.ModelUnitSystem = Rhino.UnitSystem.Millimeters
        model.Settings.ModelAbsoluteTolerance = epsilon
        model.Objects.AddBrep(source, attrs)
        if not model.Write(path, 8): raise ValueError('shared source artifact write failed')
        control = keep(source.DuplicateBrep())
        domains = op['domains']
        if domains == 'Automatic':
            success, width, height = source.Faces[face_index].GetSurfaceSize()
            if not success: raise ValueError('surface size failed')
            target = [[0., float(width)], [0., float(height)]]
        else: target = domains
        for axis, domain in enumerate(target):
            if not control.Faces[face_index].SetDomain(axis, G.Interval(*domain)):
                raise ValueError('public face SetDomain failed')
        key = (doc.Objects.AddBrep(source, attrs, None, False, False)
               if family == 'nurbs' else doc.Objects.AddBrep(source, attrs))
        if key == System.Guid.Empty: raise ValueError('source insertion failed')
        group = doc.Groups.Add('UV source group', [key])
        if group < 0: raise ValueError('source group failed')
        obj = doc.Objects.FindId(key)
        if obj.Geometry.Faces.Count != source.Faces.Count:
            raise ValueError('source admission changed the supplied face count')
        if family.startswith('box_'):
            if not obj.SelectSubObject(G.ComponentIndex(G.ComponentIndexType.BrepFace, face_index), True, True, False):
                raise ValueError('face preselection failed')
        else: doc.Objects.Select(key)
        def snapshot():
            items = list(doc.Objects)
            obj = doc.Objects.FindId(key)
            if obj is None or len(items) != 1: raise ValueError('unexpected source replacement identity/count')
            return dict(geometry=record(obj.Geometry), name=obj.Attributes.Name,
                        object_text=obj.Attributes.GetUserString('ObjectCode'),
                        geometry_text=obj.Geometry.GetUserString('GeometryCode'),
                        groups=int(obj.Attributes.GroupCount), selected=bool(obj.IsSelected(False)))
        doc.ClearUndoRecords(True)
        before = snapshot()
        macro = '_Reparameterize '+('_Automatic' if domains == 'Automatic' else ' '.join('%.17g'%x for d in domains for x in d))+' _Enter'
        marker = 'Viboceros Reparameterize '+str(System.Guid.NewGuid());Rhino.RhinoApp.WriteLine(marker)
        success, after, events = observe_command(Rhino.Commands.Command, 'Reparameterize',
            lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True)
        if Rhino.Commands.Command.InCommand(): raise ValueError('Reparameterize remains active')
        history = Rhino.RhinoApp.CommandHistoryWindowText.split(marker, 1)[-1]
        Rhino.RhinoApp.RunScript('_Undo', False);undo = snapshot()
        Rhino.RhinoApp.RunScript('_Redo', False);redo = snapshot()
        return dict(face=face_index, domains=target, success=success, before=before, after=after,
                    undo=undo, redo=redo, events=events, history=history, sdk=record(control),
                    source_admission='no_kink' if family == 'nurbs' else 'default'), 0
    finally:
        if Rhino.Commands.Command.InCommand():Rhino.RhinoApp.RunScript('!', False)
        doc.Objects.UnselectAll()
        for obj in list(doc.Objects):doc.Objects.Delete(obj.Id, True)
        group = doc.Groups.FindName('UV source group')
        if group is not None:doc.Groups.Delete(group.Index)
        doc.ModelAbsoluteTolerance = saved_tolerance
        for item in reversed(owned):item.Dispose()

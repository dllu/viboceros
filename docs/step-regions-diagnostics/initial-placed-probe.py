"""Import fixed owned STEP pole fixtures through the licensed Rhino SDK."""
import re

CASES = ('sphere', 'cone', 'sphere-pocket', 'sphere-cavity')
REGION_CASES = ('sphere-cavity', 'concentric-shells', 'nested-island',
                'torus-cavity', 'torus-hole', 'box-two-cavities', 'cone-cavity')


def validate(operation):
    region = operation.get('op') == 'step_regions'
    cases = REGION_CASES if region else CASES
    prefix = 'regions' if region else 'poles'
    if (set(operation) != {'op', 'id', 'case', 'artifact_path'}
            or operation.get('op') not in ('step_poles', 'step_regions')
            or not isinstance(operation.get('id'), (str, type(u'')))
            or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', operation['id']) is None
            or not isinstance(operation.get('case'), (str, type(u'')))
            or operation['case'] not in cases
            or not isinstance(operation.get('artifact_path'), (str, type(u'')))
            or re.match(r'^/tmp/viboceros-step-' + prefix + r'-[A-Za-z0-9_-]{1,80}/'
                        + re.escape(operation['case']) + r'\.step\Z',
                        operation['artifact_path']) is None):
        raise ValueError('invalid fixed STEP fixture')


def run(operation, host):
    validate(operation)
    Rhino = host['Rhino']
    if Rhino.Commands.Command.InCommand():
        raise ValueError('STEP pole import requires idle Rhino')
    doc = Rhino.RhinoDoc.ActiveDoc
    if list(doc.Objects):
        raise ValueError('STEP pole import requires empty owned document')
    path = 'Z:' + operation['artifact_path'].replace('/', '\\')
    try:
        options = Rhino.FileIO.FileStpReadOptions()
        options.JoinSurfaces = True
        options.LimitFaces = False
        imported = bool(Rhino.FileIO.FileStp.Read(path, doc, options))
        records = []
        top = [obj for obj in list(doc.Objects) if not obj.IsInstanceDefinitionObject]
        visited = [0]
        def capture(obj, transform, ancestry):
            visited[0] += 1
            if visited[0] > 1024 or len(ancestry) > 32:
                raise ValueError('STEP instance expansion exceeds fixture limits')
            geometry = obj.Geometry
            if isinstance(geometry, Rhino.Geometry.InstanceReferenceGeometry):
                definition = obj.InstanceDefinition
                if definition is None or str(definition.Id) in ancestry:
                    raise ValueError('missing or cyclic STEP instance definition')
                nested = transform * geometry.Xform
                for child in definition.GetObjects():
                    capture(child, nested, ancestry + (str(definition.Id),))
                return
            if not isinstance(geometry, Rhino.Geometry.Brep):
                raise ValueError('unexpected STEP import geometry: ' + str(geometry.GetType().FullName))
            placed = geometry.DuplicateBrep()
            properties = None
            try:
                if not placed.Transform(transform):
                    raise ValueError('STEP instance placement failed')
                properties = Rhino.Geometry.VolumeMassProperties.Compute(placed)
                records.append(dict(valid=bool(placed.IsValid), solid=bool(placed.IsSolid),
                    volume=None if properties is None else float(properties.Volume),
                    orientation=str(placed.SolidOrientation), instance_depth=len(ancestry),
                    geometry=host['_interchange_brep_record'](placed)))
            finally:
                if properties is not None:
                    properties.Dispose()
                placed.Dispose()
        for obj in top:
            capture(obj, Rhino.Geometry.Transform.Identity, ())
        return dict(imported=imported, objects=records, top_level_objects=len(top),
                    top_level_types=[str(obj.Geometry.GetType().FullName) for obj in top]), 0
    finally:
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)

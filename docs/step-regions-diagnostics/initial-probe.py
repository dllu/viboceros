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
        for obj in list(doc.Objects):
            geometry = obj.Geometry
            if not isinstance(geometry, Rhino.Geometry.Brep):
                raise ValueError('unexpected STEP import geometry')
            properties = Rhino.Geometry.VolumeMassProperties.Compute(geometry)
            try:
                records.append(dict(valid=bool(geometry.IsValid), solid=bool(geometry.IsSolid),
                    volume=None if properties is None else float(properties.Volume),
                    orientation=str(geometry.SolidOrientation),
                    geometry=host['_interchange_brep_record'](geometry)))
            finally:
                if properties is not None:
                    properties.Dispose()
        return dict(imported=imported, objects=records), 0
    finally:
        for obj in list(doc.Objects):
            doc.Objects.Delete(obj.Id, True)

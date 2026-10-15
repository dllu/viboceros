"""Owned public SDK mesh merges; records do not assert licensed parity."""
import json
import math
from pathlib import Path

def request():
    return json.loads(Path(__file__).with_name('fixtures').joinpath('coplanar_mesh.json').read_text())

def logical_count(mesh):
    return int(mesh.Faces.Count)-sum(len(g.FaceIndexList())-1 for g in mesh.Ngons)

def run_mesh(source,host,iterations,tolerance):
    G=host['Rhino'].Geometry
    if not isinstance(source,G.Mesh) or not source.IsValid:
        raise ValueError('coplanar merge requires a valid mesh')
    before=logical_count(source);closed=bool(source.IsClosed)
    def compute():
        output=source.DuplicateMesh()
        if output is None or output is source:raise ValueError('SDK mesh copy failed')
        try:
            changed=output.MergeAllCoplanarFaces(tolerance['absolute'],tolerance['angular'])
            after=logical_count(output)
            if (not output.IsValid or bool(output.IsClosed)!=closed or not 0<after<=before
                    or (not changed and after!=before)):
                raise ValueError('SDK coplanar mesh topology is inconsistent')
            mass=G.AreaMassProperties.Compute(output)
            if mass is None:raise ValueError('SDK mesh area failed')
            try:area=float(mass.Area)
            finally:mass.Dispose()
            volume=None
            if closed:
                mass=G.VolumeMassProperties.Compute(output)
                if mass is None:raise ValueError('SDK mesh volume failed')
                try:volume=abs(float(mass.Volume))
                finally:mass.Dispose()
            if not math.isfinite(area) or area<=0 or (closed and not math.isfinite(volume)):
                raise ValueError('Invalid SDK mesh mass properties')
            box=output.GetBoundingBox(True);bounds=[host['_xyz'](box.Min),host['_xyz'](box.Max)]
            if any(len(p)!=3 or any(not math.isfinite(x) for x in p) for p in bounds):raise ValueError('Invalid SDK mesh bounds')
            return dict(logical_face_count=after,face_count=int(output.Faces.Count),vertex_count=int(output.Vertices.Count),ngon_count=int(output.Ngons.Count),ngon_face_counts=sorted(len(g.FaceIndexList()) for g in output.Ngons),removed_faces=before-after,is_closed=closed,area=area,volume=volume,bounds=bounds,mesh=host['_polygon_mesh_value'](output))
        finally:output.Dispose()
    return host['_measure'](iterations,compute)

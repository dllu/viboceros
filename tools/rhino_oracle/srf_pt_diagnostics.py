"""Read public SDK geometry before document admission, independently of Rust."""
def record(brep,host):
    import math
    G=host['Rhino'].Geometry
    surface=brep.Faces[0].UnderlyingSurface()
    base=surface.ToBrep()
    def finite(n):
        n=float(n)
        return n if not math.isnan(n) and not math.isinf(n) else None
    def mass(geometry):
        result=G.VolumeMassProperties.Compute(geometry,True,False,False,False)
        if result is None:return None
        try:return dict(volume=finite(result.Volume),error=finite(result.VolumeError))
        finally:result.Dispose()
    try:
        result=dict(base_orientation=str(base.SolidOrientation),base_reversed=bool(base.Faces[0].OrientationIsReversed),
            base_solid=bool(base.IsSolid),base_valid=bool(base.IsValid),base=mass(base),surface=mass(surface),
            command_orientation=str(brep.SolidOrientation),command_reversed=bool(brep.Faces[0].OrientationIsReversed))
        return result
    finally:base.Dispose()

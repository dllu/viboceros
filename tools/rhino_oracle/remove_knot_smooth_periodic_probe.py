"""Use a smooth transverse basis before deforming a periodic tensor source."""
try:
    from . import remove_knot_command_probe as base
except (ImportError, ValueError):
    import remove_knot_command_probe as base

def plain(op):
    return dict(op, op='remove_knot_command')

def validate(op):
    if not isinstance(op, dict) or op.get('op') != 'remove_knot_smooth_periodic':
        raise ValueError('invalid smooth periodic removal')
    base.validate(plain(op))
    if op['family'] != 'surface' or not op.get('periodic_surface_axis') or not op.get('deform'):
        raise ValueError('requires a deformed periodic tensor source')

def run(op, host, iterations, tolerance):
    validate(op)
    R=host['Rhino'];G=R.Geometry
    owned=[]
    class SurfaceFactory:
        @staticmethod
        def CreateExtrusion(rail, vector):
            extrusion=G.Surface.CreateExtrusion(rail, vector);owned.append(extrusion)
            surface=extrusion.ToNurbsSurface();owned.append(surface)
            if not surface.IncreaseDegreeU(3) or not surface.IncreaseDegreeV(3):
                raise ValueError('smooth transverse degree preparation failed')
            return surface
    rhino=base.Forward(R);geometry=base.Forward(G);geometry.Surface=SurfaceFactory;rhino.Geometry=geometry
    local=dict(host);local['Rhino']=rhino
    try:
        value,elapsed=base.run(plain(op),local,iterations,tolerance)
        value['recipe']=op
        return value,elapsed
    finally:
        for g in reversed(owned):
            if g is not None:g.Dispose()

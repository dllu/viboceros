"""Bounded public SDK axial-spiral layout measurements, without large control dumps."""
import math
import re

def validate(op):
    if not isinstance(op, dict) or op.get('op') != 'spiral_layout' or set(op) != {'op', 'id', 'turns'}:
        raise ValueError('invalid spiral layout recipe')
    if not isinstance(op['id'], str) or re.match('\\A[A-Za-z0-9_-]{1,100}\\Z', op['id']) is None:
        raise ValueError('invalid spiral layout id')
    t = op['turns']
    if type(t) not in (int, float) or math.isnan(t) or math.isinf(t) or (not 0.001 <= abs(t) <= 1000):
        raise ValueError('spiral layout turns out of range')

def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1:
        raise ValueError('spiral layout requires one iteration')
    G = host['Rhino'].Geometry
    t = float(op['turns'])
    curve = G.NurbsCurve.CreateSpiral(G.Point3d.Origin, G.Vector3d.ZAxis, G.Point3d(2.0, 0.0, 0.0), 10.0 / abs(t), t, 2.0, 2.0)
    if curve is None:
        raise ValueError('spiral layout construction failed')
    try:
        return (dict(degree=int(curve.Degree), control_count=int(curve.Points.Count), full_knot_count=int(curve.Knots.Count) + 2), 0)
    finally:
        curve.Dispose()

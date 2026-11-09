"""Smooth Ribbon angle/scale holdouts, independent from the first corner set."""
import math
from .capture_family import capture
from .generate_ribbon import curve


def request():
    operations = []
    for angle in [15.,30.,45.,60.,75.,105.,120.,150.,165.]:
        radians = math.radians(angle)
        points = [[0.,0.,0.],[6.,0.,0.],[6.+6.*math.cos(radians),6.*math.sin(radians),0.]]
        operations.append(dict(op='ribbon_command', id='smooth-angle-'+str(angle), curve=curve(points),
            distance=.3, side=[2.,-2.,0.], both_sides=False, corner='Smooth', loose=False,
            in_cplane=False, output_layer='Current'))
    for distance in [.15,.9]:
        operations.append(dict(op='ribbon_command',id='smooth-scale-'+str(distance),
            curve=curve([[1.,2.,0.],[7.,2.,0.],[10.,6.,0.]]),distance=distance,side=[3.,0.,0.],
            both_sides=True,corner='Smooth',loose=False,in_cplane=False,output_layer='Current'))
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-6, relative=1e-9, angular=1e-6), operations=operations)


if __name__ == '__main__':
    capture('ribbon_smooth_angles', request(), ['generate_ribbon_smooth_angles.py','generate_ribbon.py',
        'ribbon_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRibbonSmoothAngles20261109',timeout=1200)

"""Public tangent availability at stationary endpoints and interior points."""
from .capture_family import capture

FAMILY='curve_tangents_stationary'


def request():
    operations=[]
    for name,xs in [('endpoint',[0.,0.,0.,1.]),('interior',[-.125,.125,-.125,.125])]:
        operations.append(dict(op='nurbs_curve_tangents',id='tangents-stationary-'+name,
            curve=dict(degree=3,knots=[0.]*4+[1.]*4,control_points=[
                dict(point=[x,0.,0.],weight=1.)for x in xs]),parameters=[0.,.5,1.,.5]))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-7,relative=1e-10,angular=1e-10),operations=operations)


if __name__=='__main__':
    capture(FAMILY,request(),['generate_curve_tangents_stationary.py','curve_tangents_probe.py',
        'geometry_validity_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleStationaryTangents20261029',timeout=300)

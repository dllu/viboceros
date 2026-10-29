"""Tangent parameters follow an explicit domain rather than authored knot values."""
import copy
from .capture_family import capture
from .generate_curve_tangents import request as tangents

FAMILY='curve_tangents_domains'


def request():
    query=tangents();original=query['operations'][0]['curve'];operations=[]
    for name,domain in [('shifted',[-3.,17.]),('narrow',[7.,7.001])]:
        source=copy.deepcopy(original);source['domain']=domain;a,b=domain
        operations.append(dict(op='nurbs_curve_tangents',id='tangents-domain-'+name,
            curve=source,parameters=[b,a,a+(b-a)*.5,b]))
    query['operations']=operations
    return query


if __name__=='__main__':
    capture(FAMILY,request(),['generate_curve_tangents_domains.py','generate_curve_tangents.py',
        'generate_loft_refit_endpoint_precision.py','fixtures/loft_refit_coarse.json',
        'fixtures/sweep1_refit_opposed_cosines.json','curve_tangents_probe.py',
        'geometry_validity_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleTangentDomains20261029',timeout=300)

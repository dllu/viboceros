"""Command-semantic insertion through the original Python geometry protocol."""
import copy
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .generate_remove_control_point import ROOT
from .structural_sdk import validate_operations


def request():
    folder=ROOT/'tools/rhino_oracle'
    q=json.loads((folder/'fixtures/insert_control_point.json').read_text())
    q['tolerance']=dict(absolute=1e-6,relative=1e-12,angular=1e-6)
    captures={r['id']:r['value']for r in json.loads((folder/'observations/insert_control_point_command.json').read_text())['results']}
    def curve(name,after=False):
        d=captures[name]['after'if after else'before'][0]['geometry']['definition']
        return copy.deepcopy({k:d[k]for k in('degree','control_points','knots')})
    def add_curve(name,c,parameter,midpoint=False):
        q['operations'].append(dict(op='curve_insert_control_point_geometry',id=name,curve=c,parameter=parameter,midpoint=midpoint))
    for family in('cubic','rational'):
        for midpoint in(False,True):
            for domain in(False,True):
                c=curve(family)
                if domain:c['domain']=[-3.,17.]
                add_curve('uniform_'+family+'_'+str(int(midpoint))+'_'+str(int(domain)),c,5. if domain else .4,midpoint)
    for midpoint in(False,True):add_curve('uniform_multispan_'+str(int(midpoint)),curve('cubic',True),.8,midpoint)
    c=curve('rational')
    for cv in c['control_points']:cv['weight']*=-8.
    add_curve('negative_gauge',c,.4)
    for side in(0.,1.):
        add_curve('uniform_end_'+str(int(side)),curve('cubic'),side)
        add_curve('nonuniform_end_'+str(int(side)),curve('seeded'),side)
    for midpoint in(False,True):add_curve('closed_seam_'+str(int(midpoint)),curve('ellipse'),0.,midpoint)
    def surface(name):
        s=captures[name]['before'][0]['geometry']['surfaces'][0]
        return dict(degree_u=s['degree'][0],degree_v=s['degree'][1],control_point_count_u=s['control_count'][0],
            control_point_count_v=s['control_count'][1],control_points=copy.deepcopy(s['control_points']),
            knots_u=list(s['knots_u']),knots_v=list(s['knots_v']))
    def add_surface(name,s,axis,parameter,midpoint=False):
        q['operations'].append(dict(s,op='surface_insert_control_point_geometry',id=name,direction=axis,parameter=parameter,midpoint=midpoint))
    def uv(s,fractions):
        return [s['knots_'+axis][s['degree_'+axis]]+(s['knots_'+axis][s['control_point_count_'+axis]]-s['knots_'+axis][s['degree_'+axis]])*f for axis,f in zip(('u','v'),fractions)]
    for family in('surface_U','surface_rational_U'):
        s=surface(family)
        for axis in('u','v'):
            for midpoint in(False,True):add_surface(family+'_'+axis+'_'+str(int(midpoint)),s,axis,uv(s,[.4,.6]),midpoint)
    s=surface('surface_U')
    for axis in('u','v'):
        add_surface('surface_boundary_'+axis,s,axis,uv(s,[0.,.6]if axis=='u'else[.4,0.]))
    s=surface('periodic_surface_U_U')
    add_surface('periodic_surface_seam',s,'u',uv(s,[0.,.5]))
    add_surface('periodic_surface_open_boundary',s,'v',uv(s,[.4,0.]))
    s=surface('surface_U')
    for axis,start,length in(('u',-3.,20.),('v',7.,4.)):
        old=s['knots_'+axis];a=old[s['degree_'+axis]];b=old[s['control_point_count_'+axis]]
        s['knots_'+axis]=[start+length*(k-a)/(b-a)for k in old]
    for axis in('u','v'):add_surface('surface_domain_'+axis,s,axis,uv(s,[.4,.6]),axis=='v')
    s=dict(degree_u=1,degree_v=1,control_point_count_u=2,control_point_count_v=2,
        control_points=[dict(point=p)for p in([0.,0.,0.],[8.,0.,0.],[0.,6.,0.],[8.,6.,2.])],knots_u=[0.,0.,1.,1.],knots_v=[0.,0.,1.,1.])
    for axis in('u','v'):add_surface('bilinear_'+axis,s,axis,[.4,.6])
    points=[[0.,0.,0.],[2.,1.,0.],[-1.,-1.,0.],[-3.,0.,0.]]
    c=dict(degree=3,control_points=[dict(point=p)for p in points],knots=[0.]*4+[1.]*4)
    add_curve('curve_point_parameter_tie',c,.5)
    s=dict(degree_u=3,degree_v=1,control_point_count_u=4,control_point_count_v=2,
        control_points=[dict(point=[p[0],p[1],z])for z in(0.,5.)for p in points],knots_u=[0.]*4+[1.]*4,knots_v=[0.,0.,1.,1.])
    add_surface('surface_point_parameter_tie',s,'u',[.5,.5])
    if len(q['operations'])!=44 or len({o['id']for o in q['operations']})!=44:raise ValueError('protocol fixture IDs changed')
    validate_operations(q)
    return q


def main():
    q=request();folder=ROOT/'tools/rhino_oracle'
    (folder/'fixtures/insert_control_point_protocol.json').write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleInsertControlProtocol20261019').run_rhino(q,timeout=600.)
    if [x['id']for x in r['results']]!=[o['id']for o in q['operations']]:raise ValueError('incomplete protocol capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_insert_control_point_protocol.py','insert_control_point_geometry_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        sources={n:digest(Path(__file__).with_name(n))for n in names},
        inputs={str(p.relative_to(ROOT)):digest(p)for p in (
            folder/'fixtures/insert_control_point.json', folder/'observations/insert_control_point_command.json')},
        request_sha256=digest(folder/'fixtures/insert_control_point_protocol.json'))
    (folder/'observations/insert_control_point_protocol.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'insertion protocol cases',flush=True)


if __name__=='__main__':main()

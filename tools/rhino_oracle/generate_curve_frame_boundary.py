"""Public SDK corner/query and seed cases, bounded independently of UI recipes."""
import hashlib,json,sys,math
from pathlib import Path
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request(kind):
    paths=[[[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[1.,1.,1.]],[[0.,0.,0.],[1.,0.,0.],[1.,0.,1.],[1.,1.,1.]],[[0.,0.,0.],[1.,2.,3.],[3.,1.,4.],[2.,3.,5.]],[[0.,0.,0.],[1.,0.,0.],[2.,1.,1.],[2.,2.,3.]]]
    ops=[]
    def add(v,p,rep='Polyline',domain=None):ops.append(dict(op='curve_frame_boundary',id=kind+'_'+str(len(ops)),vertices=v,representation=rep,parameters=p,domain=domain or[0.,3.]))
    if kind=='boundaries':
        for v in paths:
            for rep in['Polyline','Nurbs','Polycurve']:
                for p in[[0.,3.],[0.,1.,2.,3.],[0.,.9,1.8,3.],[1.,2.,3.]]:add(v,p,rep)
        for v in paths:add(v,[0.,1.,2.,3.],'NurbsCubic')
        for exponent in[-10,-20,-28,-30,-31,-32,-33,-40,-60]:
            e=2.**exponent;add([[0.,0.,0.],[1.,0.,0.],[1.+e,0.,1.],[1.+e,1.,1.]],[0.,1.,2.,3.])
    elif kind=='seeds':
        directions=[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.],[-1.,0.,0.],[0.,-1.,0.],[0.,0.,-1.],[1.,1.,0.],[1.,0.,1.],[0.,1.,1.],[1.,1.,1.],[1.,2.,3.],[-1.,-2.,-3.],[1.,1.,2.],[1.,2.,1.],[2.,1.,1.]]
        directions += [[1.,1.,sign*2.**exponent]for exponent in[-20,-28,-30,-31,-32,-33,-40]for sign in[1.,-1.]]
        for v in directions:add([[0.,0.,0.],v],[0.,1.],domain=[0.,1.])
        for rep in['Bezier','BezierChain']:
            for p in[[0.,1.],[0.,.5,1.]]:add([[0.,0.,0.],[1.,0.,0.],[2.,1.,1.],[3.,1.,0.]],p,rep,[0.,1.])
        chain=[[0.,0.,0.],[1.,0.,0.],[2.,1.,1.],[3.,1.,0.],[3.,2.,0.],[3.,3.,1.],[4.,4.,1.],[5.,4.,1.],[5.,5.,2.],[6.,5.,3.]]
        for p in[[0.,3.],[0.,1.,2.,3.],[0.,.5,1.,1.5,2.,2.5,3.],[0.,.9,1.8,3.]]:add(chain,p,'BezierChain')
        # Parameter scaling does not change corner frames.
        for domain in[[-7.,2.],[0.,2.**-20],[0.,2.**19]]:
            a,b=domain;add(paths[2],[a+(b-a)*i/3. for i in range(4)],'Nurbs',domain)
        for exponent in[-40,-60,-100,-200,-500]:
            e=2.**exponent;add([[-1.,0.,0.],[0.,0.,0.],[e,0.,1.],[e,1.,1.]],[0.,1.,2.,3.])
        add([[-1.,0.,0.],[-2./3.,0.,0.],[-1./3.,0.,0.],[0.,0.,0.],[0.,0.,1.],[0.,1.,2.],[1.,2.,3.]],[0.,1.,2.],'BezierChain',[0.,2.])
    elif kind=='proximity':
        v=paths[0]
        for rep in['Polyline','Nurbs','Polycurve']:
            for delta in[0.,math.ulp(2.),2.*math.ulp(2.),8.*math.ulp(2.),1e-12,1e-9,1e-6]:
                for sign in[1.,-1.]:add(v,[0.,2.+sign*delta,3.],rep)
        for t in[.5e-8,1e-8,1.49e-8,2e-8,math.sqrt(2.)*2.**-26,2.2e-8,3e-8,1e-7]:add([[0.,0.,0.],[1.,1.,t]],[0.,1.],domain=[0.,1.])
        chain=[[0.,0.,0.],[1.,0.,0.],[2.,1.,1.],[3.,1.,0.],[3.,2.,0.],[3.,3.,1.],[4.,4.,1.],[5.,4.,1.],[5.,5.,2.],[6.,5.,3.]]
        add(chain,[0,1,2,3],'BezierChain');ops[-1]['query_mode']='Knots'
        fallback=[[-1.,0.,0.],[-2./3.,0.,0.],[-1./3.,0.,0.],[0.,0.,0.],[0.,0.,1.],[0.,1.,2.],[1.,2.,3.]]
        add(fallback,[0,1,2],'BezierChain',[0.,2.]);ops[-1]['query_mode']='Knots'
        knot=0.40866056036878556 # Exact public SDK span boundary retained from the first capture.
        add(fallback,[0.,knot,(knot+2.)/2.,2.],'BezierChain',[0.,2.])
        add(fallback,[0.,knot,knot+(2.-knot)*.1,(knot+2.)/2.,2.],'BezierChain',[0.,2.])
    else:raise ValueError('unknown frame capture suite')
    assert 1<=len(ops)<=64
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-9,relative=1e-12,angular=1e-10),operations=ops);validate_operations(q);return q

def main():
    from .client import OracleClient
    kind=sys.argv[1];q=request(kind);name='curve_frame_'+kind;path=HERE/'fixtures'/(name+'.json');path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_curve_frame_boundary.py','curve_frame_boundary_probe.py','rhino_worker.py','structural_sdk.py','client.py'];digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={n:digest(HERE/n)for n in names}
    print('Capturing',len(q['operations']),kind,'public frame batches',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleFrame'+kind.title()+'20261020').run_rhino(q,timeout=240)
    assert sources=={n:digest(HERE/n)for n in names};r['provenance']=dict(request_sha256=digest(path),sources=sources);(HERE/'observations'/(name+'.json')).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print(kind,'frames recorded',flush=True)
if __name__=='__main__':main()

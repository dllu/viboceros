"""Representable microscopic rational spans, native creator and history cases."""


def cases():
    result=[]
    for plane in ('xz','xy'):
        for width in ('original','ulp'):
            for height in (-5.,5.):
                for both in (False,True):
                    controls=[[2.,0.,0.],[3.,0.,0.],[4.,0.,0.],[4.,0.,10.],[5.,0.,0.],[6.,0.,0.],[7.,0.,0.]]
                    if plane=='xy':controls=[[x,z,y]for x,y,z in controls]
                    end=0.50000000000001 if width=='original' else 0.5+2.**-53
                    curve=dict(degree=2,control_points=[dict(point=p,weight=w)for p,w in zip(controls,[1.,.75,1.,.5,1.,.75,1.])],knots=[0.,0.,0.,.5,.5,end,end,1.,1.,1.])
                    result.append(dict(op='short_span_extrusions',id=plane+'_'+width+'_'+('n'if height<0 else'p')+str(int(both)),curve=curve,height=height,both_sides=both))
    return result


def validate(op):
    valid=isinstance(op,dict)and op in cases()
    if valid:
        curve=op['curve']
        numeric=lambda v:isinstance(v,(int,float))and not isinstance(v,bool)
        valid=(type(op['both_sides'])is bool and numeric(op['height']) and type(curve['degree'])is int
               and all(numeric(k)for k in curve['knots'])
               and all(numeric(c['weight'])and all(numeric(v)for v in c['point'])for c in curve['control_points']))
    if not valid:raise ValueError('fixed owned short-span extrusion recipe required')

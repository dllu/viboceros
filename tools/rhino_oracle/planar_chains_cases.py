"""Owned open-curve boundary chains, branches and closure thresholds."""
from copy import deepcopy
if __package__:
    from .planar_regions_cases import polygon,rectangle,circle
else:
    from planar_regions_cases import polygon,rectangle,circle


def polyline(points):
    n=len(points)
    return dict(degree=1,control_points=[dict(point=deepcopy(p),weight=1.)for p in points],
                knots=[0.]+[float(i)for i in range(n)]+[float(n-1)])


def edges(a,b,c,d,z=0.):
    p=[[a,b,z],[c,b,z],[c,d,z],[a,d,z],[a,b,z]]
    return [polyline([p[i],p[i+1]])for i in range(4)]


def pilot():
    rows=[];outer=edges(0.,0.,10.,10.);hole=edges(2.,2.,6.,6.)
    def add(name,curves):rows.append(dict(op='planar_chains',id='chains_'+name,curves=deepcopy(curves)))
    add('rectangle',outer);add('shuffled',[outer[i]for i in [2,0,3,1]])
    reverse=deepcopy(outer)
    for i in [0,2]:reverse[i]['control_points'].reverse()
    add('reversed',reverse)
    add('polyline_and_line',[polyline([[0.,0.,0.],[10.,0.,0.],[10.,10.,0.],[0.,10.,0.]]),outer[3]])
    add('two_polylines',[polyline([[0.,0.,0.],[10.,0.,0.],[10.,10.,0.]]),polyline([[10.,10.,0.],[0.,10.,0.],[0.,0.,0.]])])
    c=circle(0.,0.,4.)
    quarters=[dict(degree=2,control_points=deepcopy(c['control_points'][2*i:2*i+3]),knots=[0.,0.,0.,1.,1.,1.])for i in range(4)]
    add('quarters',quarters);add('quarter_shuffle',[quarters[i]for i in [2,0,3,1]])
    half=dict(degree=2,control_points=deepcopy(c['control_points'][:5]),knots=[0.,0.,0.,1.,1.,2.,2.,2.])
    add('semicircle',[half,polyline([[-4.,0.,0.],[4.,0.,0.]])])
    add('open_outer_closed_hole',outer+[rectangle(2.,2.,6.,6.)])
    add('closed_outer_open_hole',[rectangle(0.,0.,10.,10.)]+hole)
    add('both_open',outer+hole)
    add('disconnected',outer+edges(12.,0.,16.,4.))
    add('nested',outer+hole+edges(3.,3.,5.,5.))
    add('two_planes',outer+edges(0.,0.,10.,10.,2.))
    add('tail',outer+[polyline([[0.,0.,0.],[-2.,0.,0.]])])
    add('diagonal',outer+[polyline([[0.,0.,0.],[10.,10.,0.]])])
    add('touching_cycles',outer+edges(10.,10.,14.,14.))
    add('shared_side',outer+edges(10.,0.,14.,10.))
    add('duplicate_edge',outer+[outer[0]])
    add('duplicate_cycle',outer+outer)
    add('incomplete',outer[:3]);add('single_line',[outer[0]])
    add('closed_and_incomplete',[rectangle(0.,0.,10.,10.)]+edges(12.,0.,16.,4.)[:3])
    nonplanar=deepcopy(outer)
    nonplanar[1]['control_points'][1]['point'][2]=1.;nonplanar[2]['control_points'][0]['point'][2]=1.
    add('nonplanar',nonplanar)
    for factor in [.5,1.,1.01,1.5,2.,2.1,2.11,3.]:
        curves=deepcopy(outer);curves[-1]['control_points'][-1]['point'][0]=factor*1e-6
        add('gap_'+str(factor).replace('.','_'),curves)
    for factor in [.5,2.,3.]:
        add('single_near_closed_'+str(factor).replace('.','_'),[polyline([[0.,0.,0.],[10.,0.,0.],[10.,10.,0.],[0.,10.,0.],[factor*1e-6,0.,0.]])])
    return rows


def validate(op):
    def same(a,b):
        if isinstance(b,dict):return isinstance(a,dict)and set(a)==set(b)and all(same(a[k],v)for k,v in b.items())
        if isinstance(b,list):return isinstance(a,list)and len(a)==len(b)and all(same(x,y)for x,y in zip(a,b))
        if isinstance(b,(float,int)):return type(a)in (float,int)and a==b
        return type(a)==type(b)and a==b
    owned=next((r for r in pilot()+holdouts()+contexts()+salvage()+filled()+modes()+networks()+network_holdouts()+network_crossings()if isinstance(op,dict)and r['id']==op.get('id')),None)
    if owned is None or not same(op,owned):raise ValueError('fixed owned planar-chain recipe required')


def holdouts():
    rows=[];outer=edges(0.,0.,10.,10.);a=edges(2.,2.,6.,6.);b=edges(4.,4.,8.,8.)
    def add(name,curves):rows.append(dict(op='planar_chains',id='chains_holdout_'+name,curves=deepcopy(curves)))
    add('overlap_pair',a+b);add('closed_outer_overlap',[rectangle(0.,0.,10.,10.)]+a+b)
    add('open_outer_overlap',outer+a+b)
    add('crossed_bars',edges(1.,4.,9.,6.)+edges(4.,1.,6.,9.))
    add('partial_shared_side',outer+edges(10.,4.,14.,8.))
    add('closed_touching',[rectangle(0.,0.,10.,10.),rectangle(10.,0.,14.,10.)])
    add('interior_line',outer+[polyline([[0.,5.,0.],[10.,5.,0.]])])
    add('interior_path',outer+[polyline([[0.,0.,0.],[5.,7.,0.],[10.,10.,0.]])])
    add('bridge_disjoint',outer+edges(12.,0.,16.,4.)+[polyline([[10.,0.,0.],[12.,0.,0.]])])
    add('bridge_nested',outer+a+[polyline([[0.,0.,0.],[2.,2.,0.]])])
    add('nonplanar_tail',outer+[polyline([[0.,0.,0.],[0.,0.,5.]])])
    vertical=[deepcopy(c)for c in edges(0.,0.,10.,10.)]
    for c in vertical:
        for p in c['control_points']:x,y,z=p['point'];p['point']=[x,z,y]
    add('orthogonal_touching',outer+vertical)
    box=outer+edges(0.,0.,10.,10.,10.)
    box+=[polyline([[x,y,0.],[x,y,10.]])for x,y in [(0.,0.),(10.,0.),(10.,10.),(0.,10.)]]
    add('box_edges',box)
    for axis in ['y','xy']:
        for factor in [.5,1.,1.01,1.5,2.1,2.11,3.]:
            curves=deepcopy(outer);end=curves[-1]['control_points'][-1]['point'];end[1]=factor*1e-6
            if axis=='xy':end[0]=factor*1e-6
            add('gap_'+axis+'_'+str(factor).replace('.','_'),curves)
    return rows


def contexts():
    rows=[]
    chosen=[r for r in pilot()if r['id']in ['chains_rectangle','chains_quarters','chains_semicircle','chains_both_open','chains_tail','chains_touching_cycles','chains_gap_3_0','chains_single_near_closed_3_0']]
    for source in chosen:
        for label in ['tilted','small','large','domain','gauge']:
            row=deepcopy(source);row['id']=source['id']+'_'+label
            for i,c in enumerate(row['curves']):
                if label=='domain':c['knots']=[123.+i+(i+.25)*k for k in c['knots']]
                for p in c['control_points']:
                    if label=='gauge':p['weight']*=7.
                    x,y,z=p['point']
                    if label=='tilted':p['point']=[.6*x+.8*z+31.,y-17.,-.8*x+.6*z+9.]
                    if label=='small':p['point']=[.001*x+2.,.001*y-3.,.001*z+4.]
                    if label=='large':p['point']=[1000.*x+1e6,1000.*y-2e6,1000.*z+3e6]
            rows.append(row)
    def add(name,curves):rows.append(dict(op='planar_chains',id='chains_context_'+name,curves=deepcopy(curves)))
    for dy in [.5e-6,.1e-6,1e-9,1e-12]:
        curves=edges(0.,0.,10.,10.);curves[-1]['control_points'][-1]['point']=[3e-6,dy,0.]
        add('asymmetric_'+str(dy),curves)
    for end in [[-2.,-2.,0.],[0.,-2.,0.],[2.,2.,0.],[5.,5.,0.],[5.,7.,0.]]:
        add('branch_'+str(end[0])+'_'+str(end[1]),edges(0.,0.,10.,10.)+[polyline([[0.,0.,0.],end])])
    add('circle_outer_open_hole',[circle(0.,0.,10.)]+edges(2.,2.,6.,6.))
    add('circle_outer_overlapping_open_holes',[circle(0.,0.,10.)]+edges(2.,2.,6.,6.)+edges(4.,4.,8.,8.))
    q=next(r for r in pilot()if r['id']=='chains_quarters')['curves']
    add('rectangle_outer_quarter_hole',[rectangle(-10.,-10.,10.,10.)]+q)
    return rows


def salvage():
    rows=[];outer=edges(0.,0.,10.,10.);hole=edges(2.,2.,6.,6.)
    def add(name,curves):rows.append(dict(op='planar_chains',id='chains_salvage_'+name,curves=deepcopy(curves)))
    add('lollipop_alone',hole+[polyline([[0.,0.,0.],[2.,2.,0.]])])
    tail=outer+[polyline([[0.,0.,0.],[-2.,-2.,0.]])]
    add('tail_with_closed_region',tail+[rectangle(20.,0.,24.,4.)])
    gap=deepcopy(hole);gap[-1]['control_points'][-1]['point'][0]+=3e-6
    add('closed_outer_open_gap',[rectangle(0.,0.,10.,10.)]+gap)
    add('open_gap_with_closed_hole',deepcopy(next(r for r in pilot()if r['id']=='chains_gap_3_0')['curves'])+[circle(5.,5.,1.)])
    add('bridge_with_closed_region',outer+edges(12.,0.,16.,4.)+[polyline([[10.,0.,0.],[12.,0.,0.]])]+[rectangle(20.,0.,24.,4.)])
    quarters=next(r for r in pilot()if r['id']=='chains_quarters')['curves']
    add('tangent_tail_first',[polyline([[4.,0.,0.],[4.,2.,0.]])]+quarters)
    add('tangent_tail_last',quarters+[polyline([[4.,0.,0.],[4.,2.,0.]])])
    add('tangent_tail_first_closed_other',[polyline([[4.,0.,0.],[4.,2.,0.]])]+quarters+[rectangle(20.,0.,24.,4.)])
    return rows


def filled():
    rows=[]
    def gap(a,b,c,d,z=0.):
        curves=edges(a,b,c,d,z);curves[-1]['control_points'][-1]['point'][0]+=3e-6;return curves
    def add(name,curves):rows.append(dict(op='planar_chains',id='chains_filled_'+name,curves=deepcopy(curves)))
    add('partial_overlap',[rectangle(0.,0.,6.,6.)]+gap(4.,2.,10.,8.))
    add('within_hole',[rectangle(0.,0.,10.,10.),rectangle(2.,2.,6.,6.)]+gap(3.,3.,5.,5.))
    add('covers_hole',[rectangle(0.,0.,10.,10.),rectangle(2.,2.,6.,6.)]+gap(1.,1.,7.,7.))
    add('crosses_hole',[rectangle(0.,0.,10.,10.),rectangle(2.,2.,6.,6.)]+gap(4.,4.,8.,8.))
    add('different_plane',[rectangle(0.,0.,10.,10.)]+gap(2.,2.,6.,6.,2.))
    add('outside_tangent',[rectangle(0.,0.,10.,10.)]+gap(10.,2.,14.,6.))
    return rows


def modes():
    rows=[];closed=[rectangle(0.,0.,10.,10.),rectangle(2.,2.,6.,6.)]
    def add(name,curves):rows.append(dict(op='planar_chains',id='chains_mode_'+name,curves=deepcopy(curves)))
    add('isolated_line',closed+[polyline([[20.,0.,0.],[21.,0.,0.]])])
    add('incomplete',closed+edges(20.,0.,24.,4.)[:3])
    add('attached_line',closed+[polyline([[0.,0.,0.],[-2.,-2.,0.]])])
    add('two_segments',closed+[polyline([[20.,0.,0.],[22.,0.,0.],[22.,2.,0.]])])
    def gap(a,b,c,d,z):
        result=edges(a,b,c,d,z);result[-1]['control_points'][-1]['point'][0]+=3e-6;return result
    add('different_plane_reversed',gap(2.,2.,6.,6.,2.)+[closed[0]])
    add('two_partial_planes',gap(0.,0.,10.,10.,0.)+gap(2.,2.,6.,6.,2.))
    add('hole_and_other_plane',closed+gap(2.,2.,6.,6.,2.))
    add('hole_and_incomplete_other_plane',closed+[polyline([[20.,0.,2.],[22.,0.,2.],[22.,2.,2.]])])
    return rows


def networks():
    rows=[]
    def add(name,curves):rows.append(dict(op='planar_chains',id='chains_network_'+name,curves=deepcopy(curves)))
    outer=rectangle(0.,0.,10.,10.)
    for gap in [0.,.5e-6,1e-6,1.01e-6,3e-6,1e-4,.1,1.,3.9,4.,5.]:
        curves=edges(10.,2.,14.,6.);curves[-1]['control_points'][-1]['point'][0]+=gap
        add('tangent_'+str(gap),[outer]+curves)
    u=edges(10.,2.,14.,6.)[:3]
    add('u', [outer]+u)
    add('u_polyline',[outer,polyline([[10.,2.,0.],[14.,2.,0.],[14.,6.,0.],[10.,6.,0.]])])
    add('u_first',u+[outer])
    add('u_open_outer',edges(0.,0.,10.,10.)+u)
    add('u_two_sides',[outer]+u[:2])
    add('u_with_hole',[outer,rectangle(2.,2.,6.,6.)]+u)
    add('u_other_plane',[outer,rectangle(2.,2.,6.,6.)]+[rectangle(0.,0.,10.,10.,2.)]+[polyline([[10.,2.,2.],[14.,2.,2.],[14.,6.,2.],[10.,6.,2.]])])
    add('bridge_one',[outer,rectangle(12.,0.,16.,4.),polyline([[10.,0.,0.],[12.,0.,0.]])])
    add('bridge_two',[outer,rectangle(12.,0.,16.,4.),polyline([[10.,0.,0.],[12.,0.,0.]]),polyline([[10.,4.,0.],[12.,4.,0.]])])
    add('circle_u',[circle(0.,0.,10.),polyline([[0.,-10.,0.],[14.,-10.,0.],[14.,10.,0.],[0.,10.,0.]])])
    add('u_isolated_line',[outer]+u+[polyline([[20.,0.,0.],[22.,0.,0.]])])
    for shift in [-1.01e-6,-1e-6,-.5e-6,.5e-6,1e-6,1.01e-6]:
        add('u_end_offset_'+str(shift),[outer,polyline([[10.+shift,2.,0.],[14.,2.,0.],[14.,6.,0.],[10.+shift,6.,0.]])])
    for label in ['tilted','small','large','domain','gauge']:
        row=deepcopy(rows[6]);row['id']='chains_network_context_'+label
        for i,c in enumerate(row['curves']):
            if label=='domain':c['knots']=[123.+i+(i+.25)*k for k in c['knots']]
            for p in c['control_points']:
                if label=='gauge':p['weight']*=7.
                x,y,z=p['point']
                if label=='tilted':p['point']=[.6*x+.8*z+31.,y-17.,-.8*x+.6*z+9.]
                if label=='small':p['point']=[.001*x+2.,.001*y-3.,.001*z+4.]
                if label=='large':p['point']=[1000.*x+1e6,1000.*y-2e6,1000.*z+3e6]
        rows.append(row)
    return rows


def network_holdouts():
    rows=[];outer=rectangle(0.,0.,10.,10.);hole=rectangle(2.,2.,6.,6.)
    def add(name,curves):rows.append(dict(op='planar_chains',id='chains_network_holdout_'+name,curves=deepcopy(curves)))
    gap=edges(20.,0.,24.,4.);gap[-1]['control_points'][-1]['point'][0]+=.1
    u=edges(10.,2.,14.,6.)[:3]
    add('recovered_and_u',[outer]+u+gap)
    add('recovered_and_u_polyline',[outer,polyline([[10.,2.,0.],[14.,2.,0.],[14.,6.,0.],[10.,6.,0.]])]+gap)
    for name,line in [('isolated',[[4.,8.,0.],[6.,8.,0.]]),('tail',[[0.,8.,0.],[5.,8.,0.]]),('chord',[[0.,8.,0.],[10.,8.,0.]]),('through_hole',[[0.,4.,0.],[10.,4.,0.]])]:
        add('closed_'+name,[outer,hole,polyline(line)])
        add('recovered_'+name,[outer,hole,polyline(line)]+gap)
    other=edges(24.,4.,28.,8.);other[-1]['control_points'][-1]['point'][0]+=.1
    add('touching_recovered',gap+other)
    add('recovered_and_closed_touch',gap+[rectangle(24.,4.,28.,8.)])
    quarters=next(r for r in pilot()if r['id']=='chains_quarters')['curves'];tail=polyline([[4.,0.,0.],[4.,2.,0.]])
    add('rational_tail_point_touch',[tail]+quarters+[rectangle(4.,0.,8.,4.)])
    add('rational_tail_nested',[tail]+quarters+[rectangle(-1.,-1.,1.,1.)])
    add('rational_tail_outside',[tail]+quarters+[rectangle(20.,0.,24.,4.)])
    add('bridge_nested_with_tail',edges(0.,0.,10.,10.)+edges(2.,2.,6.,6.)+[polyline([[0.,0.,0.],[2.,2.,0.]])])
    add('near_tangent_with_closed_hole',[outer,hole]+deepcopy(next(r for r in networks()if r['id']=='chains_network_tangent_0.1')['curves'][1:]))
    return rows


def network_crossings():
    rows=[]
    def add(name,curves):rows.append(dict(op='planar_chains',id='chains_network_crossing_'+name,curves=deepcopy(curves)))
    for delta in [5.,6.,10.]:
        gap=edges(10.,2.,14.,6.);gap[-1]['control_points'][-1]['point'][0]+=delta
        add('solo_'+str(delta),gap)
    add('outer_10',[rectangle(0.,0.,10.,10.)]+gap)
    bow=[[0.,0.,0.],[4.,4.,0.],[0.,4.,0.],[4.,0.,0.]]
    add('bow_polyline',[polyline(bow)]);add('bow_reversed',[polyline(list(reversed(bow)))])
    add('bow_lines',[polyline(bow[i:i+2])for i in range(3)])
    add('bow_with_hole',[rectangle(-2.,-2.,6.,6.),rectangle(1.8,3.2,2.2,3.6),polyline(bow)])
    add('two_bows',[polyline(bow+[[8.,0.,0.],[12.,4.,0.],[8.,4.,0.],[12.,0.,0.]])])
    for dy in [.5e-6,.1e-6,1e-9]:
        gap=edges(10.,2.,14.,6.);gap[-1]['control_points'][-1]['point']=[10.+3e-6,2.+dy,0.]
        add('asymmetric_'+str(dy),[rectangle(0.,0.,10.,10.)]+gap)
    return rows

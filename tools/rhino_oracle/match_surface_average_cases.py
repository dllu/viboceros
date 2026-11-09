"""Independent two-sided surface matches, with explicit traversal choices."""
import copy
if __package__:
    from .match_surface_cases import FINAL,case,surface,chart_edit
else:
    from match_surface_cases import FINAL,case,surface,chart_edit

PILOT={}
for kind in ['polynomial','rational']:
    for mode in ['Position','Tangency','Curvature']:
        for first,second in [(False,False),(False,True),(True,False),(True,True)]:
            value=copy.deepcopy(FINAL['match_f_'+kind+'_'+mode.lower()])
            value.update(average=True,reverse_match=first,reverse_target=second)
            key='average_'+kind+'_'+mode.lower()+'_'+str(int(first))+str(int(second))
            PILOT[key]=value
CASES=copy.deepcopy(PILOT)

HOLDOUTS={}
for mode in ['Position','Tangency']:
    base=copy.deepcopy(FINAL['match_f_rational_'+mode.lower()])
    base.update(average=True,reverse_match=True,reverse_target=False)
    for label,changes in [
        ('keep',dict(iso='Preserve')),('perpendicular',dict(iso='Perpendicular')),
        ('target_iso',dict(iso='MatchTarget')),('preserve_position',dict(other_end='Position')),
        ('preserve_tangency',dict(other_end='Tangency')),('preserve_curvature',dict(other_end='Curvature')),
        ('closest',dict(closest_points=True)),('refine',dict(refine=True)),
    ]:
        value=copy.deepcopy(base);value.update(changes)
        HOLDOUTS['average_h_'+mode.lower()+'_'+label]=value
    for a,b in [(1,3),(2,5)]:
        HOLDOUTS['average_h_'+mode.lower()+'_degrees_'+str(a)+'_'+str(b)]=case(mode,
            sources=[surface(degree=a,rational=True),surface(True,degree=b,rational=True)],
            average=True,reverse_match=True,reverse_target=False)
    for label,a,b,side,target_side in [
        ('source_west','reverse_u',None,'West','West'),
        ('source_north','transpose',None,'North','West'),
        ('source_south','transpose_reverse_v',None,'South','West'),
        ('target_east',None,'reverse_u','East','East'),
        ('target_south',None,'transpose','East','South'),
        ('target_north',None,'transpose_reverse_v','East','North'),
        ('source_reverse_v','reverse_v',None,'East','West'),
        ('both_transpose','transpose','transpose','North','South'),
    ]:
        value=copy.deepcopy(base);value.update(side=side,target_side=target_side)
        for i,operation in enumerate([a,b]):
            if operation:
                for op in (['transpose','reverse_v']if operation=='transpose_reverse_v'else[operation]):
                    value['sources'][i]=chart_edit(value['sources'][i],op)
        HOLDOUTS['average_h_'+mode.lower()+'_'+label]=value
CASES.update(copy.deepcopy(HOLDOUTS))


def request(names=None):
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),
        operations=[dict(op='average_surface_sdk',id=name,case=name)for name in (CASES if names is None else names)])

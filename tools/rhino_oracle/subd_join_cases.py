"""Bounded control-net joins, shared verbatim by native and Rust replays."""
from copy import deepcopy


def quad(x=0.,y=0.,gap=0.,reverse=False,rise=0.):
    face=[0,1,2,3]
    if reverse:face=[0,3,2,1]
    return dict(points=[[x+gap,y,0.],[x+4.+gap,y,rise],[x+4.+gap,y+3.,rise],[x+gap,y+3.,0.]],faces=[face])


def strip():
    return dict(points=[[0.,0.,0.],[4.,0.,0.],[8.,0.,0.],[0.,3.,0.],[4.,3.,0.],[8.,3.,0.]],faces=[[0,1,4,3],[1,2,5,4]],creases=[[1,4]],corners=[0])


SCENES={
    'single':[quad()],
    'adjacent':[quad(),quad(4.)],
    'bent':[quad(),quad(4.,rise=3.)],
    'reversed':[quad(),quad(4.,reverse=True)],
    'disjoint':[quad(),quad(12.)],
    'point':[quad(),quad(4.,y=3.)],
    'gap_half':[quad(),quad(4.,gap=.0005)],
    'gap_equal':[quad(),quad(4.,gap=.001)],
    'gap_beyond':[quad(),quad(4.,gap=.0015)],
    'chain':[quad(),quad(8.),quad(4.)],
    'duplicate':[quad(),quad()],
    'threeway':[quad(),quad(4.),quad(4.,rise=3.)],
    'retained_tags':[strip(),quad(8.)],
}


def case(name,crease=False):
    if name not in SCENES:raise ValueError('unknown fixed SubD join scene')
    return dict(sources=deepcopy(SCENES[name]),crease=crease,tolerance=.001)


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','scene','crease'}
            or op['op']!='subd_join_sdk' or not isinstance(op['scene'],str) or op['scene'] not in SCENES
            or not isinstance(op['crease'],bool)
            or op['id']!=op['scene']+('-crease' if op['crease'] else '-smooth')):
        raise ValueError('fixed SubD join SDK recipe required')

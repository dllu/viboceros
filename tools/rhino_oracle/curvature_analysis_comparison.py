"""Strict command-state comparison without assuming identical tessellations."""
import math
from .curvature_analysis_probe import STYLES,validate
from .client import OracleProtocolError

def canonical(op,value,native):
    validate(op)
    def require(ok,reason):
        if not ok:raise OracleProtocolError('curvature analysis '+reason)
    def finite(v):
        if isinstance(v,dict):return all(finite(x) for x in v.values())
        if isinstance(v,list):return all(finite(x) for x in v)
        return type(v) is not float or math.isfinite(v)
    def settings(s):
        return (isinstance(s,dict) and set(s)=={'style','ranges'} and s['style'] in STYLES
                and isinstance(s['ranges'],list) and len(s['ranges'])==4
                and all(isinstance(r,list) and len(r)==2 and all(type(x) in (int,float) for x in r) for r in s['ranges']))
    require(isinstance(value,dict) and finite(value),'nonfinite or invalid response')
    require(set(value)==({'defaults','states','traces'} if native else {'states','settings'}),'response fields')
    states=value['states'];count=len(op['steps'])+1
    require(isinstance(states,list) and len(states)==count,'state count')
    for s in states:
        require(isinstance(s,dict) and set(s)=={'exists','selected','hidden','analysis','draft'} and all(type(x) is bool for x in s.values()),'state fields')
        require(s['exists'] or not any(s.values()),'deleted state')
        require(not (s['analysis'] and s['draft']),'conflicting surface color modes')
    if not native:
        require(isinstance(value['settings'],list) and len(value['settings'])==count and all(s is None or settings(s) for s in value['settings']),'settings')
        return value
    require(settings(value['defaults']),'defaults')
    traces=value['traces'];require(isinstance(traces,list) and len(traces)==count-1,'trace count')
    options=[None];enabled=False
    for step,t in zip(op['steps'],traces):
        require(isinstance(t,dict) and set(t)=={'command','success','events','settings','meshes'} and t['command']==step and t['success'] is True,'command trace')
        require(isinstance(t['events'],list) and all(isinstance(e,dict) and set(e)=={'name','result'} and all(isinstance(x,str) for x in e.values()) for e in t['events']),'events')
        events=[e for e in t['events'] if e['name']==step]
        require((not t['events'] if step=='MaxRange' else len(events)==1 and events[0]['result']=='Success'),'completion')
        require(settings(t['settings']),'native settings')
        if step=='CurvatureAnalysis':enabled=True
        elif step=='CurvatureAnalysisOff':enabled=False
        options.append(t['settings'] if enabled else None)
        require(isinstance(t['meshes'],list),'meshes')
        for m in t['meshes']:
            require(isinstance(m,dict) and set(m)=={'vertex_count','samples'} and type(m['vertex_count']) is int and m['vertex_count']>0 and isinstance(m['samples'],list),'mesh fields')
            indices=[]
            for s in m['samples']:
                require(isinstance(s,dict) and set(s)=={'index','uv','principal','color'} and type(s['index']) is int and 0<=s['index']<m['vertex_count'],'sample fields')
                indices.append(s['index'])
                require(all(isinstance(s[k],list) and len(s[k])==2 and all(type(x) in (int,float) for x in s[k]) for k in ('uv','principal')),'surface sample')
                require(isinstance(s['color'],list) and len(s['color'])==4 and all(type(x) is int and 0<=x<=255 for x in s['color']) and s['color'][3]==255,'color sample')
            require(indices==sorted(set(indices)),'sample indices')
    return dict(states=states,settings=options)

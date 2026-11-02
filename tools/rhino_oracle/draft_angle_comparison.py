"""Validate native evidence and compare model/display state independently of tessellation."""
import math
from .draft_angle_probe import validate
from .client import OracleProtocolError

def canonical(op,value,native):
    validate(op)
    def require(ok,message):
        if not ok:raise OracleProtocolError('draft-angle '+message)
    def finite(v):
        if isinstance(v,dict):return all(finite(x) for x in v.values())
        if isinstance(v,list):return all(finite(x) for x in v)
        return type(v) is not float or math.isfinite(v)
    def valid_settings(s):
        return (isinstance(s,dict) and set(s)=={'range','up','wires'}
                and isinstance(s['range'],list) and len(s['range'])==2
                and isinstance(s['up'],list) and len(s['up'])==3
                and all(type(x) in (int,float) for x in s['range']+s['up'])
                and any(s['up']) and type(s['wires']) is bool)
    require(isinstance(value,dict) and finite(value),'invalid/nonfinite response')
    require(set(value)==({'defaults','initial','states','traces'} if native else {'states','settings'}),'response fields')
    states=value['states'];count=len(op['steps'])+1
    require(isinstance(states,list) and len(states)==count,'state count')
    for s in states:
        require(isinstance(s,dict) and set(s)=={'exists','selected','hidden','analysis'} and all(type(v) is bool for v in s.values()),'state fields')
        require(s['exists'] or not any(s.values()),'deleted state')
    if not native:
        require(isinstance(value['settings'],list) and len(value['settings'])==count,'settings count')
        for s in value['settings']:
            if s is not None:
                require(valid_settings(s),'settings fields')
        return value
    traces=value['traces'];require(isinstance(traces,list) and len(traces)==count-1,'trace count')
    require(valid_settings(value['defaults']) and valid_settings(value['initial']),'initial settings')
    options=[None];enabled=False
    for step,t in zip(op['steps'],traces):
        require(isinstance(t,dict) and set(t)=={'command','success','events','settings','meshes'} and t['command']==step and t['success'] is True,'command trace')
        require(isinstance(t['events'],list) and all(isinstance(e,dict) and set(e)=={'name','result'} and all(isinstance(x,str) for x in e.values()) for e in t['events']),'events')
        events=[e for e in t['events'] if e.get('name')==step]
        require((not t['events'] if step=='SelNone' else len(events)==1 and events[0]['result']=='Success'),'completion event')
        s=t['settings'];require(valid_settings(s),'native settings')
        if step=='DraftAngleAnalysis':enabled=True
        elif step=='DraftAngleAnalysisOff':enabled=False
        options.append(dict(range=[math.degrees(x) for x in s['range']],up=s['up'],wires=s['wires']) if enabled else None)
        require(isinstance(t['meshes'],list),'mesh records')
        for m in t['meshes']:
            require(isinstance(m,dict) and set(m)=={'vertex_count','indices','normals','colors'} and type(m['vertex_count']) is int and m['vertex_count']>0,'mesh fields')
            require(isinstance(m['indices'],list) and m['indices']==sorted(set(m['indices'])) and all(type(i) is int and 0<=i<m['vertex_count'] for i in m['indices']),'mesh indices')
            require(isinstance(m['normals'],list) and isinstance(m['colors'],list) and len(m['indices'])==len(m['normals'])==len(m['colors'])>0,'mesh sample count')
            for n,c in zip(m['normals'],m['colors']):
                require(isinstance(n,list) and len(n)==3 and all(type(x) in (int,float) for x in n) and abs(sum(x*x for x in n)-1)<1e-5,'mesh normal')
                require(isinstance(c,list) and len(c)==4 and all(type(x) is int and 0<=x<=255 for x in c) and c[3]==255,'mesh color')
    return dict(states=states,settings=options)

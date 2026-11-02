"""Validate native UI/command traces and compare complete analysis/model states."""
import math
from .client import OracleProtocolError
from .direction_analysis_probe import validate

def settings(controls):
    if not isinstance(controls,list):raise OracleProtocolError('invalid direction analysis controls')
    wanted={}
    for row in controls:
        if not isinstance(row,dict) or set(row)!={'label','kind','check'} or not isinstance(row['label'],str) or not isinstance(row['kind'],str):raise OracleProtocolError('invalid direction control record')
        label=row['label'].replace('&','')
        if label in ('Direction','U','V') and row['kind']=='Button':
            if label in wanted or type(row['check']) is not int or row['check'] not in (0,1):raise OracleProtocolError('invalid direction checkbox state')
            wanted[label]=bool(row['check'])
    if not wanted:
        if controls:raise OracleProtocolError('direction panel controls missing')
        return None
    if set(wanted)!={'Direction','U','V'}:raise OracleProtocolError('incomplete direction panel controls')
    return {k.lower():v for k,v in wanted.items()}

def canonical(operation,value,native):
    validate(operation)
    if not isinstance(value,dict) or set(value)!=({'states','traces'} if native else {'states','settings'}):raise OracleProtocolError('invalid direction analysis response')
    states=value['states'];count=len(operation['steps'])+1
    if not isinstance(states,list) or len(states)!=count:raise OracleProtocolError('invalid direction analysis state count')
    def finite(tree):
        if isinstance(tree,dict):return all(isinstance(k,str) and finite(v) for k,v in tree.items())
        if isinstance(tree,list):return all(finite(v) for v in tree)
        if type(tree) is float:return math.isfinite(tree)
        return tree is None or type(tree) in (bool,int,str)
    for state in states:
        rows=state if operation.get('companion',False) else [state]
        if not isinstance(rows,list) or len(rows)!=(2 if operation.get('companion',False) else 1):raise OracleProtocolError('invalid direction source count')
        for row in rows:
            if (not isinstance(row,dict) or set(row)!={'exists','selected','hidden','analysis','geometry'}
                    or any(type(row[k]) is not bool for k in ('exists','selected','hidden','analysis')) or not finite(row)
                    or (row['exists'] and row['geometry'] is None) or (not row['exists'] and row!={'exists':False,'selected':False,'hidden':False,'analysis':False,'geometry':None})):
                raise OracleProtocolError('invalid direction model state')
    if native:
        traces=value['traces']
        if not isinstance(traces,list) or len(traces)!=count-1:raise OracleProtocolError('invalid direction trace count')
        options=[None]
        for step_index,(step,trace) in enumerate(zip(operation['steps'],traces)):
            fields={'command','macro','success','events','controls','modes'}|({'input'} if 'picks' in operation else set())
            if (not isinstance(trace,dict) or set(trace)!=fields
                    or trace['command']!=step or type(trace['success']) is not bool or not trace['success']
                    or not isinstance(trace['events'],list) or not isinstance(trace['modes'],list)):
                raise OracleProtocolError('direction command/control did not succeed')
            for mode in trace['modes']:
                if mode!={'id':'83141aa3-563b-4355-8009-48685119bc02','name':'Object direction analysis'}:raise OracleProtocolError('unexpected direction analysis mode')
            primary=states[step_index+1][0] if operation.get('companion',False) else states[step_index+1]
            if len(trace['modes'])!=int(primary['analysis']):raise OracleProtocolError('inconsistent direction analysis membership')
            names=[e.get('name') for e in trace['events'] if isinstance(e,dict)]
            if 'input' in trace:
                inputs=trace['input'];expected_index=operation['picks'].get(str(step_index),0)
                if (not isinstance(inputs,dict) or set(inputs)!={'target','target_index','sent','errors','ended'}
                        or inputs['target_index']!=expected_index or inputs['errors'] or not isinstance(inputs['sent'],list)
                        or len(inputs['sent'])>9 or len(inputs['sent'])%3==1):raise OracleProtocolError('invalid owned direction input trace')
                for index,sent in enumerate(inputs['sent']):
                    if (not isinstance(sent,dict) or set(sent)!={'token','prompt'} or not isinstance(sent['prompt'],str)
                            or sent['token']!=('_SelID',inputs['target'],'')[index%3]):raise OracleProtocolError('direction selection token mismatch')
                if [(e['name'],e['result']) for e in inputs['ended']] != [(e['name'],e['result']) for e in trace['events']]:raise OracleProtocolError('direction input command trace mismatch')
            if step in ('UReverse','VReverse','SwapUV','Flip'):
                expected='Flip' if step=='Flip' else 'Dir'
                if [name for name in names if name!='SelID']!=['GetObjectProxy',expected] or trace['macro'] is not None:raise OracleProtocolError('direction edit did not use the native panel route')
            elif '=' in step or step in ('SelectPrimary','SelectCompanion','SelNone'):
                if trace['events'] or trace['macro'] is not None:raise OracleProtocolError('invalid direction control trace')
            elif step in ('Add','Remove'):
                if [name for name in names if name!='SelID']!=['GetObjectProxy']:raise OracleProtocolError('invalid direction membership trace')
            else:
                if names.count(step)!=1 or any(name not in (step,'SelID') for name in names):raise OracleProtocolError('direction command must finish once')
            for event in trace['events']:
                if event.get('result') not in ('Success','Nothing'):raise OracleProtocolError('direction command failed')
            options.append(settings(trace['controls']))
    else:
        options=value['settings']
        if not isinstance(options,list) or len(options)!=count:raise OracleProtocolError('invalid direction settings count')
        for row in options:
            if row is not None and (not isinstance(row,dict) or set(row)!={'direction','u','v'} or any(type(v) is not bool for v in row.values())):raise OracleProtocolError('invalid direction display flags')
    return dict(states=states,settings=options)

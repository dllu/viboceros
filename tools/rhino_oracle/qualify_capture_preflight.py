"""Version-bound source equivalence audit for the preflight extraction.

This historical check is run at the refactor revision, not during future
implementation work that intentionally extends the driver.
"""
import ast
import copy
import gzip
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace

from . import client, capture_preflight
from .oracle_errors import OracleError, OracleProtocolError

HERE=Path(__file__).resolve().parent
LEGACY_SHA256='edbfdcafca6e7723010c7b0fa1b9c02cc0f948a32711ba077d1a26428d38f337'
STAGES=[('geometry','validate_sources'),('geometry','prepare'),('construction','prepare'),
        ('surfaces','prepare'),('components','prepare'),('curves','prepare'),
        ('modeling','prepare'),('picking','prepare')]


def legacy_tree():
    source=gzip.decompress((HERE/'observations/source_snapshots'/(LEGACY_SHA256+'.gz')).read_bytes())
    if hashlib.sha256(source).hexdigest()!=LEGACY_SHA256:raise ValueError('legacy client digest')
    return ast.parse(source)


def run_node(tree):
    return next(f for c in tree.body if isinstance(c,ast.ClassDef)and c.name=='OracleClient'
                for f in c.body if isinstance(f,ast.FunctionDef)and f.name=='run_rhino')


def legacy_preflight():
    body=run_node(legacy_tree()).body
    start=next(i+1 for i,n in enumerate(body)if isinstance(n,ast.Expr)and isinstance(n.value,ast.Call)
               and getattr(n.value.func,'id',None)=='validate_structural_sdk_operations')
    stop=next(i for i,n in enumerate(body)if isinstance(n,ast.If)and isinstance(n.test,ast.UnaryOp)
              and isinstance(n.test.operand,ast.Call)and isinstance(n.test.operand.func,ast.Attribute)
              and n.test.operand.func.attr=='is_file'and getattr(n.test.operand.func.value,'id',None)=='worker_source')
    return body[start:stop]


def key(node):return ast.dump(node,include_attributes=False)


class RestoreClientName(ast.NodeTransformer):
    def visit_Name(self,node):
        return ast.copy_location(ast.Name(id='self',ctx=node.ctx),node)if node.id=='client'else node


def shared_guards():
    return {key(n.test):n for n in legacy_preflight()if isinstance(n,ast.If)
            and len(n.body)>1 and isinstance(n.body[0],ast.Assign)and isinstance(n.body[1],ast.If)
            and any(isinstance(x,ast.Constant)and isinstance(x.value,str)and 'private dedicated'in x.value
                    for x in ast.walk(n.body[1]))}


def expanded_stages():
    guards=shared_guards();result=[];worker=next(n for n in legacy_preflight()
        if isinstance(n,ast.Assign)and any(getattr(t,'id',None)=='worker_source'for t in n.targets))
    for i,(module,name)in enumerate(STAGES):
        tree=ast.parse((HERE/('capture_preflight_'+module+'.py')).read_text())
        fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name==name)
        returned=fn.body[-1].value if isinstance(fn.body[-1],ast.Return)else None
        expected=['interaction','worker_source']if module=='picking'else ['interaction']
        names=[getattr(v,'id',None)for v in returned.elts]if isinstance(returned,ast.Tuple)else [getattr(returned,'id',None)]
        if names!=expected:
            raise AssertionError('stage must pass the interaction to its successor')
        for n in fn.body[:-1]:
            n=RestoreClientName().visit(copy.deepcopy(n))
            if isinstance(n,ast.If)and n.body and isinstance(n.body[0],ast.Assign):
                call=n.body[0].value
                if isinstance(call,ast.Call)and getattr(call.func,'id',None)=='require_dedicated_capture':
                    old=guards[key(n.test)]
                    family=old.test.args[0].elt.comparators[0].value
                    message=old.body[1].body[0].exc.args[0].value
                    if [getattr(a,'id',None)for a in call.args[:2]]!=['self','request']:
                        raise AssertionError('private guard receives a different request/client')
                    if [ast.literal_eval(a)for a in call.args[2:]]!=[family,message]:
                        raise AssertionError('private guard family/message differs')
                    keywords={k.arg:ast.literal_eval(k.value)for k in call.keywords}
                    if keywords!=({'strict_iterations':False}if family=='typed_selection'else {}):
                        raise AssertionError('legacy iteration-type policy differs')
                    n.body[:1]=copy.deepcopy(old.body[:2])
            result.append(n)
        if i==0:result.append(copy.deepcopy(worker))
    return result


def legacy_function(body=None, *, with_worker=False):
    body=copy.deepcopy(legacy_preflight()if body is None else body)
    result=ast.Return(value=ast.Tuple(elts=[ast.Name(id=n,ctx=ast.Load())for n in ['interaction','worker_source']],ctx=ast.Load())if with_worker else ast.Name(id='interaction',ctx=ast.Load()))
    fn=ast.FunctionDef(name='legacy',args=ast.arguments(posonlyargs=[],args=[ast.arg(arg='self'),ast.arg(arg='request')],
        kwonlyargs=[],kw_defaults=[],defaults=[]),body=body+[result],decorator_list=[])
    ns=dict(vars(client));ns.update(OracleProtocolError=OracleProtocolError,Path=Path,__file__=str(HERE/'client.py'),__package__=__package__)
    exec(compile(ast.fix_missing_locations(ast.Module(body=[fn],type_ignores=[])),'frozen_preflight','exec'),ns)
    return ns['legacy']


def verify():
    assert [key(n) for n in expanded_stages()] == [key(n) for n in legacy_preflight()], "preflight bodies/order changed"
    old=legacy_tree(); new=ast.parse((HERE/'client.py').read_text())
    before=run_node(old); after=run_node(new)
    positions=[i for i,n in enumerate(after.body)if isinstance(n,ast.Expr)and isinstance(n.value,ast.Call)
               and getattr(n.value.func,'id',None)=='validate_capture_sources']
    assert len(positions)==1, "source validation delegation changed"
    index=positions[0]; validation,worker,preparation=after.body[index:index+3]
    assert [getattr(a,'id',None)for a in validation.value.args]==['self','request']and not validation.value.keywords
    old_worker=next(n for n in legacy_preflight()if isinstance(n,ast.Assign)and any(getattr(t,'id',None)=='worker_source'for t in n.targets))
    assert key(worker)==key(old_worker), "default worker construction changed"
    assert isinstance(preparation,ast.Assign)and len(preparation.targets)==1
    assert isinstance(preparation.targets[0],ast.Tuple)and [getattr(n,'id',None)for n in preparation.targets[0].elts]==['interaction','worker_source']
    call=preparation.value
    assert isinstance(call,ast.Call)and getattr(call.func,'id',None)=='prepare_capture'
    assert [getattr(a,'id',None)for a in call.args]==['self','request','worker_source']and not call.keywords
    after.body=after.body[:index]+copy.deepcopy(legacy_preflight())+after.body[index+3:]
    assert key(before)==key(after), "launch/desktop/response flow changed"
    old_functions={n.name:key(n) for n in old.body if isinstance(n,(ast.FunctionDef,ast.ClassDef)) and n.name not in ['OracleError','OracleProtocolError','OracleClient']}
    new_functions={n.name:key(n) for n in new.body if isinstance(n,(ast.FunctionDef,ast.ClassDef)) and n.name!='OracleClient'}
    assert old_functions==new_functions, "host helper bodies changed"
    old_client=next(n for n in old.body if isinstance(n,ast.ClassDef) and n.name=='OracleClient')
    new_client=next(n for n in new.body if isinstance(n,ast.ClassDef) and n.name=='OracleClient')
    assert key(old_client)==key(new_client), "other client methods changed"
    assert [key(n)for n in old.body if isinstance(n,ast.Assign)]==[key(n)for n in new.body if isinstance(n,ast.Assign)], "host constants changed"
    errors=ast.parse((HERE/'oracle_errors.py').read_text())
    assert {n.name:key(n)for n in old.body if isinstance(n,ast.ClassDef)and n.name in ['OracleError','OracleProtocolError']}=={
        n.name:key(n)for n in errors.body if isinstance(n,ast.ClassDef)}, "exception definitions changed"
    return dict(legacy_client_sha256=LEGACY_SHA256,original_preflight_statements=len(legacy_preflight()),
                private_guards_factored=9,preflight_order_and_bodies_identical=True,
                launch_response_desktop_and_helper_bodies_identical=True)


if __name__=='__main__':
    print(json.dumps(verify(),indent=2))

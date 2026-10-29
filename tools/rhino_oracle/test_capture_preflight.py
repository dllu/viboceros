"""Request-policy regressions against frozen pre-refactor behavior."""
import ast
import copy
from contextlib import ExitStack
import json
from types import SimpleNamespace
import unittest
from unittest import mock

from . import client, capture_preflight
from .capture_preflight_common import require_dedicated_capture
from .oracle_errors import OracleError, OracleProtocolError
from .qualify_capture_preflight import (HERE, STAGES, key, legacy_function,
                                         shared_guards, RestoreClientName)


class CapturePreflightTests(unittest.TestCase):
    def test_dispatch_order_and_public_exception_aliases(self):
        events=[]
        with ExitStack()as stack:
            for i,(module,name)in enumerate(STAGES[1:]):
                def stage(c,r,interaction=None,worker_source=None,i=i):
                    events.append((i,interaction));return (i,worker_source)if i==6 else i
                stack.enter_context(mock.patch.object(getattr(capture_preflight,module),name,side_effect=stage))
            self.assertEqual(capture_preflight.prepare_capture(SimpleNamespace(settings_scheme='Owned'),{},HERE/'rhino_worker.py'),(6,HERE/'rhino_worker.py'))
        self.assertEqual(events,[(0,None)]+[(i,i-1)for i in range(1,7)])
        self.assertIs(client.OracleError,OracleError);self.assertIs(client.OracleProtocolError,OracleProtocolError)
        self.assertTrue(issubclass(OracleProtocolError,OracleError))

    def test_shared_private_guards_match_legacy_boundary_and_type_policy(self):
        shared=shared_guards();checked=0;missing=object()
        module=ast.parse((HERE/'capture_preflight_components.py').read_text())
        function=next(n for n in module.body if isinstance(n,ast.FunctionDef))
        for n in function.body:
            if not isinstance(n,ast.If)or not n.body or not isinstance(n.body[0],ast.Assign):continue
            call=n.body[0].value
            if not isinstance(call,ast.Call)or getattr(call.func,'id',None)!='require_dedicated_capture':continue
            test_key=key(RestoreClientName().visit(copy.deepcopy(n.test)))
            if test_key not in shared:continue
            old=shared[test_key];family=ast.literal_eval(call.args[2]);message=ast.literal_eval(call.args[3])
            block=copy.deepcopy(old.body[:2]);block.append(ast.Assign(targets=[ast.Name(id='interaction',ctx=ast.Store())],value=ast.Name(id='operations',ctx=ast.Load())))
            previous=legacy_function(block)
            for scheme in [None,'Owned']:
                for iterations in [missing,None,0,1,2,True,False,1.0,'1',[]]:
                    for count in [0,1,64,65]:
                        for mixed in [False,True]:
                            q=dict(operations=[dict(op=family,id=str(i))for i in range(count)])
                            if iterations is not missing:q['iterations']=iterations
                            if mixed and count:q['operations'][-1]['op']='other'
                            c=SimpleNamespace(settings_scheme=scheme)
                            def outcome(fn):
                                try:return ('ok',fn())
                                except Exception as e:return(type(e),str(e))
                            expected=outcome(lambda:previous(c,q))
                            actual=outcome(lambda:require_dedicated_capture(c,q,family,message,strict_iterations=family!='typed_selection'))
                            self.assertEqual(actual,expected,(family,scheme,iterations,count,mixed));checked+=1
        self.assertEqual(checked,9*2*10*4*2)

    def test_representative_real_fixture_outcomes_match_without_launching_rhino(self):
        previous=legacy_function(with_worker=True);selected=[]
        names=['curve_array','merge_faces_command','mesh_face_collapse','pt_off_selected','scale_positions',
               'soft_crease','subd_face_custom_holdouts','subdivide','typed_selection','unjoin_edge_command']
        for name in names:selected.append(HERE/'fixtures'/(name+'.json'))
        for pattern in ['arc*.json','surface_rebuild*.json','boolean_union*.json','mesh_trim*.json','view_camera*.json','group_picking*.json','point_snap*.json']:
            selected.extend(sorted((HERE/'fixtures').glob(pattern))[:2])
        successes=0
        def outcome(fn,q):
            try:
                obj,worker=fn(SimpleNamespace(settings_scheme='Owned'),q)
                return ('ok',None if obj is None else(type(obj).__name__, tuple(c.__name__ for c in type(obj).__bases__)),worker)
            except Exception as e:return(type(e),str(e))
        with mock.patch.object(client,'_run_logged',side_effect=AssertionError('unexpected launch')):
            for path in selected:
                q=json.loads(path.read_text())
                if 'operations'not in q:continue
                old_query=copy.deepcopy(q);new_query=copy.deepcopy(q)
                expected=outcome(previous,old_query);actual=outcome(lambda c,q:capture_preflight.prepare_capture(c,q,HERE/'rhino_worker.py'),new_query)
                self.assertEqual(actual,expected,path.name);self.assertEqual(old_query,new_query,path.name)
                successes+=actual[0]=='ok'
        self.assertGreaterEqual(successes,10)


    def test_private_guard_lookup_and_short_circuit_order_match_legacy(self):
        guards=shared_guards()
        for node in ast.parse((HERE/'capture_preflight_components.py').read_text()).body:
            if not isinstance(node,ast.FunctionDef):continue
            for n in node.body:
                if not isinstance(n,ast.If)or not n.body or not isinstance(n.body[0],ast.Assign):continue
                call=n.body[0].value
                if not isinstance(call,ast.Call)or getattr(call.func,'id',None)!='require_dedicated_capture':continue
                test_key=key(RestoreClientName().visit(copy.deepcopy(n.test)))
                if test_key not in guards:continue
                old=guards[test_key];family=ast.literal_eval(call.args[2]);message=ast.literal_eval(call.args[3])
                block=copy.deepcopy(old.body[:2]);block.append(ast.Assign(targets=[ast.Name(id='interaction',ctx=ast.Store())],value=ast.Name(id='operations',ctx=ast.Load())))
                previous=legacy_function(block)
                for scheme in [None,'Owned']:
                    for iterations in [True,1,2]:
                        traces=[]
                        for current in [False,True]:
                            trace=[]
                            class Request(dict):
                                def __getitem__(self,k):trace.append(('item',k));return super().__getitem__(k)
                                def get(self,k,default=None):trace.append(('get',k));return super().get(k,default)
                            class Client:
                                @property
                                def settings_scheme(self):trace.append(('scheme',));return scheme
                            q=Request(operations=[dict(op=family)],iterations=iterations)
                            try:
                                if current:require_dedicated_capture(Client(),q,family,message,strict_iterations=family!='typed_selection')
                                else:previous(Client(),q)
                            except OracleProtocolError:pass
                            traces.append(trace)
                        self.assertEqual(traces[0],traces[1],(family,scheme,iterations))

    def test_every_host_helper_is_in_the_shared_source_inventory(self):
        from .qualification_sources import ORACLE_CLIENT_MODULES,with_shared_sources
        self.assertEqual(len(capture_preflight.CLIENT_SOURCE_FILES),len(set(capture_preflight.CLIENT_SOURCE_FILES)))
        pending=['client.py'];resolved=set()
        while pending:
            name=pending.pop()
            if name in resolved:continue
            resolved.add(name)
            for n in ast.walk(ast.parse((HERE/name).read_text())):
                if not isinstance(n,ast.ImportFrom)or n.level!=1:continue
                modules=[n.module]if n.module else [a.name for a in n.names]
                for module in modules:
                    if module=='oracle_errors'or module.startswith('capture_preflight'):
                        pending.append(module+'.py')
        self.assertEqual(resolved,set(capture_preflight.CLIENT_SOURCE_FILES))
        for name in ORACLE_CLIENT_MODULES:
            self.assertTrue((HERE.parent.parent/name).is_file(),name)
            self.assertTrue(set(ORACLE_CLIENT_MODULES)<=set(with_shared_sources([name])),name)


    def test_angle_callback_has_the_same_timing_and_mouse_actions(self):
        from . import capture_preflight_picking as picking
        request={'operations':[{'op':'plane_primitive','primitive':'ArcCenterEndpoint'}]}
        traces=[]
        for previous in [True,False]:
            with mock.patch.object(picking.time,'sleep')as sleep,mock.patch.object(picking.subprocess,'run')as run:
                if previous:picker,worker=legacy_function(with_worker=True)(SimpleNamespace(settings_scheme='Owned'),copy.deepcopy(request))
                else:picker,worker=capture_preflight.prepare_capture(SimpleNamespace(settings_scheme='Owned'),copy.deepcopy(request),HERE/'rhino_worker.py')
                self.assertTrue(picker.send_input('case','20','30','123'))
                self.assertEqual(worker,HERE/'rhino_worker.py')
                self.assertEqual(sleep.call_count,2);self.assertEqual(run.call_count,2)
                traces.append((sleep.call_args_list,run.call_args_list))
        self.assertEqual(traces[0],traces[1])

    def test_specialized_worker_choices_survive_preflight_return(self):
        from . import group_picking,undo_selection
        previous=legacy_function(with_worker=True)
        with mock.patch.object(group_picking,'validate_request'),mock.patch.object(undo_selection,'validate_request'):
            for family,script in [('group_picking','group_picking_worker.py'),('undo_selection','undo_selection_worker.py')]:
                request={'operations':[{'op':family,'id':'one'}]};c=SimpleNamespace(settings_scheme='Owned')
                old_picker,old_worker=previous(c,copy.deepcopy(request))
                picker,worker=capture_preflight.prepare_capture(c,copy.deepcopy(request),HERE/'rhino_worker.py')
                self.assertEqual(worker,HERE/script);self.assertEqual(worker,old_worker)
                self.assertEqual(type(picker).__name__,type(old_picker).__name__)

    def test_nested_picker_globals_are_resolved_by_the_extracted_modules(self):
        import builtins
        import symtable
        for module,_ in STAGES:
            target=getattr(capture_preflight,module)
            table=symtable.symtable((HERE/('capture_preflight_'+module+'.py')).read_text(),target.__file__,'exec')
            unresolved=set()
            def visit(scope):
                unresolved.update(s.get_name()for s in scope.get_symbols()if s.is_global()
                                  and s.get_name()not in vars(target)and s.get_name()not in vars(builtins))
                for child in scope.get_children():visit(child)
            visit(table)
            self.assertEqual(unresolved,set(),module)

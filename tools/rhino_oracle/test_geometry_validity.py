"""SDK ownership, strict stored-record admission and real worker routing mocks."""
import ast
import copy
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase,mock
from .geometry_validity_probe import request,validate,run


class ValidityTests(TestCase):
    def test_fixtures_and_stored_layout_are_strict_before_native_access(self):
        for operation in request()['operations']: validate(operation)
        original=next(op for op in request()['operations'] if op['id']=='coincident_triangle')
        for updates in [dict(extra=True),dict(op='check'),dict(source=None)]:
            op=copy.deepcopy(original);op.update(updates)
            with self.subTest(updates=updates),self.assertRaises(ValueError):validate(op)
        for key,value in [('vertices',[[True,0,0]]*3),('vertices',[[float('nan'),0,0]]*3),
                          ('vertices',[[float('inf'),0,0]]*3),('faces',[[0,1,3]]),
                          ('faces',[[0,0,1]]),('faces',[[0,1,True]]),('faces',[])]:
            op=copy.deepcopy(original);op['source'][key]=value
            with self.subTest(key=key,value=value),self.assertRaises(ValueError):validate(op)

    def test_numeric_source_fields_do_not_coerce_booleans_or_numeric_strings(self):
        for source in [dict(type='point', point=[True, 0, 0]),
                       dict(type='line', start=['1', 0, 0], end=[2, 0, 0]),
                       dict(type='nurbs', degree=1.0),
                       dict(type='surface', degree_u=True),
                       dict(type='circle', radius=True)]:
            op=dict(op='geometry_validity', id='bad', source=source)
            with self.subTest(source=source), self.assertRaises(ValueError): validate(op)

    def host(self,valid=True):
        source=mock.Mock();output=mock.Mock()
        source.Duplicate.return_value=output
        output.IsValidWithLog.return_value=(valid,'' if valid else 'bad face')
        host={'_object_source':mock.Mock(return_value=source),
              '_measure':lambda n,f:([f() for _ in range(n)][-1],0)}
        return host,source,output

    def test_owned_public_sdk_calls_valid_invalid_and_no_log_fallback(self):
        for valid in (True,False):
            host,source,output=self.host(valid)
            value,_=run(request()['operations'][0],host,2,request()['tolerance'])
            self.assertEqual(value,dict(status='valid' if valid else 'invalid',has_diagnostics=not valid))
            self.assertEqual(source.Duplicate.call_count,2)
            self.assertEqual(output.IsValidWithLog.call_count,2)
            self.assertEqual(output.Dispose.call_count,2)
            source.Dispose.assert_called_once()
        host,source,output=self.host(False);output.IsValidWithLog.return_value=(False,'')
        self.assertTrue(run(request()['operations'][0],host,1,request()['tolerance'])[0]['has_diagnostics'])

    def test_copy_and_check_failures_dispose_only_owned_geometry(self):
        for stage in ('copy','alias','check','source'):
            host,source,output=self.host()
            if stage=='copy':source.Duplicate.return_value=None
            elif stage=='alias':source.Duplicate.return_value=source
            elif stage=='check':output.IsValidWithLog.side_effect=ValueError('native failure')
            else:host['_object_source'].return_value=None
            with self.subTest(stage=stage),self.assertRaises(ValueError):
                run(request()['operations'][0],host,1,request()['tolerance'])
            if stage!='source':source.Dispose.assert_called_once()
            if stage=='check':output.Dispose.assert_called_once()
            else:output.Dispose.assert_not_called()

    def test_stored_mesh_worker_uses_double_vertices_skips_only_final_validity_and_disposes_failures(self):
        tree=ast.parse(Path(__file__).with_name('rhino_worker.py').read_text())
        nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in ('_polygon_mesh','_object_source')]
        scope={};exec(compile(ast.Module(body=nodes,type_ignores=[]),'<mesh source>','exec'),scope)
        for valid in (True,False):
            mesh=mock.Mock(IsValid=valid)
            mesh.Vertices.Add.return_value=0;mesh.Faces.AddFace.return_value=0
            scope.update(Rhino=NS(Geometry=NS(Mesh=lambda:mesh)),_point=lambda p:p)
            definition=next(op['source'] for op in request()['operations'] if op['id']=='coincident_triangle')
            self.assertIs(scope['_object_source'](definition,{}),mesh)
            self.assertTrue(mesh.Vertices.UseDoublePrecisionVertices)
            mesh.Dispose.assert_not_called()
            mesh.Faces.AddFace.side_effect=ValueError('allocation failure')
            with self.assertRaises(ValueError):scope['_object_source'](definition,{})
            mesh.Dispose.assert_called_once()
        # Existing generated-mesh callers retain their original admission gate.
        mesh=mock.Mock(IsValid=False);mesh.Vertices.Add.return_value=0;mesh.Faces.AddFace.return_value=0
        scope.update(Rhino=NS(Geometry=NS(Mesh=lambda:mesh)))
        with self.assertRaisesRegex(ValueError,'mesh is invalid'):
            scope['_polygon_mesh'](definition['vertices'],definition['faces'])
        mesh.Dispose.assert_called_once()

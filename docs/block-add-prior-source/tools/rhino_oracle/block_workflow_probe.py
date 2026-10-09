# -*- coding: utf-8 -*-
"""Owned public-SDK block workflows with UUID-independent graph records."""
import math
import re
from fractions import Fraction

MAX_HANDLES = 4096
MAX_RECORDS = 65536
try:
    string_types = (basestring,)
except NameError:
    string_types = (str,)


def _fold(name):
    return ''.join(chr(ord(c) + 32) if 'A' <= c <= 'Z' else c for c in name)


def _name(value):
    if (not isinstance(value, string_types) or not value or value.strip() != value
            or len(value.encode('utf-8')) > 128 or '\0' in value):
        raise ValueError('invalid block workflow name')
    return _fold(value)


def _numbers(value, count):
    if (not isinstance(value, list) or len(value) != count or
            any(isinstance(v, bool) or not isinstance(v, (int, float)) or
                math.isnan(v) or math.isinf(v) for v in value)):
        raise ValueError('invalid finite block coordinates')


def _attributes(value):
    if not isinstance(value, dict) or set(value) - set(('name', 'color', 'color_source', 'user_text', 'geometry_user_text')):
        raise ValueError('invalid block attributes')
    if value.get('name') is not None and not isinstance(value['name'], string_types):
        raise ValueError('invalid block object name')
    if value.get('color') is not None and (not isinstance(value['color'], list) or len(value['color']) != 3 or
            any(type(v) is not int or not 0 <= v <= 255 for v in value['color'])):
        raise ValueError('invalid block color')
    if value.get('color_source', 'layer') not in ('layer', 'object', 'parent'):
        raise ValueError('invalid block color source')
    for key in ('user_text', 'geometry_user_text'):
        pairs = value.get(key, {})
        if (not isinstance(pairs, dict) or any(not isinstance(k, string_types) or not k or '\0' in k or
                not isinstance(v, string_types) or '\0' in v for k, v in pairs.items())):
            raise ValueError('invalid block user text')


def _user_text_record(pairs, owned_ids):
    result=dict(pairs)
    marker='$block-instance-original-object-id$'
    if marker in result and result[marker].lower() in owned_ids:
        del result[marker]
    return result


def validate(operation):
    """Resolve handle lifetimes, nested graphs and output budgets before host access."""
    sources, steps = operation.get('sources'), operation.get('steps')
    if not isinstance(sources, list) or not 1 <= len(sources) <= 32 or not isinstance(steps, list) or not 1 <= len(steps) <= 64:
        raise ValueError('invalid block workflow sizes')
    allowed = ('point', 'point_cloud', 'mesh', 'line', 'polyline', 'arc', 'circle', 'ellipse', 'nurbs', 'polycurve', 'surface', 'brep')
    if any(not isinstance(s, dict) or s.get('type') not in allowed for s in sources):
        raise ValueError('invalid block geometry source')
    specs = operation.get('attributes', [])
    if not isinstance(specs, list) or (specs and len(specs) != len(sources)):
        raise ValueError('invalid block source attributes')
    for spec in specs:
        _attributes(spec)
    if type(operation.get('record_groups', False)) is not bool:
        raise ValueError('invalid block group recording')
    if type(operation.get('record_management', False)) is not bool:
        raise ValueError('invalid block management recording')
    if type(operation.get('record_states',False)) is not bool:raise ValueError('invalid block state recording')
    handles = [None] * len(sources)  # None means ordinary geometry; absent means deleted.
    alive = set(range(len(sources)))
    protected = set()
    definitions = {}
    records = 0

    def leaves(name, stack):
        if name in stack or len(stack) >= 64:
            raise ValueError('cyclic or excessive-depth block graph')
        result = []
        for member in definitions[name]:
            result.extend([None] if member is None else leaves(member, stack + [name]))
            if len(result) > MAX_HANDLES:
                raise ValueError('block workflow output budget exceeded')
        return result

    def record_cost():
        cost = sum(1 if handles[i] is None else 1 + len(leaves(handles[i], [])) for i in alive)
        return cost + sum(len(members) for members in definitions.values())

    records += record_cost()
    for step in steps:
        if not isinstance(step, dict):
            raise ValueError('invalid block workflow step')
        action = step.get('action')
        if action == 'create':
            if set(step) - set(('action', 'name', 'base', 'sources','api')) or not set(('name','base','sources')).issubset(step) or step.get('api','sdk') not in ('sdk','command'):
                raise ValueError('invalid block create fields')
            name = _name(step['name'])
            if step.get('api','sdk')=='command' and (name in definitions or not re.match(r'^[A-Za-z0-9_-]+$',step['name'])):
                raise ValueError('command block creation needs a new plain-token name')
            _numbers(step['base'], 3)
            picks = step['sources']
            if (not isinstance(picks, list) or not picks or any(type(i) is not int or i not in alive for i in picks)
                    or len(set(picks)) != len(picks)):
                raise ValueError('invalid live block handles')
            definitions[name] = [handles[i] for i in sorted(picks)]
            for key in definitions:
                leaves(key, [])
            alive.difference_update(picks)
            outputs = [name]
        elif action == 'insert':
            if set(step) - set(('action', 'name', 'transform', 'attributes')) or not set(('name', 'transform')).issubset(step):
                raise ValueError('invalid block insert fields')
            name = _name(step['name'])
            if name not in definitions:
                raise ValueError('undefined block name')
            matrix = step['transform']
            if not isinstance(matrix, list) or len(matrix) != 4:
                raise ValueError('invalid block matrix')
            for row in matrix:
                _numbers(row, 4)
            if matrix[3] != [0, 0, 0, 1]:
                raise ValueError('block matrix must be affine')
            # Exact dyadic arithmetic also admits nonsingular subnormal scales.
            a = [[Fraction.from_float(float(v)) for v in row[:3]] for row in matrix[:3]]
            determinant = sum(a[0][i] * (a[1][(i+1) % 3] * a[2][(i+2) % 3] - a[1][(i+2) % 3] * a[2][(i+1) % 3]) for i in range(3))
            if determinant == 0:
                raise ValueError('singular block matrix')
            _attributes(step.get('attributes', {}))
            outputs = [name]
        elif action == 'explode':
            if (set(step) - set(('action', 'object', 'recursive', 'api', 'group_output')) or 'object' not in step
                    or type(step.get('recursive', False)) is not bool or step.get('api', 'command') not in ('sdk', 'command')):
                raise ValueError('invalid block explode fields')
            if (type(step.get('group_output', False)) is not bool or
                    step.get('group_output', False) and (not step.get('recursive', False) or step.get('api', 'command') != 'command')):
                raise ValueError('GroupOutput requires recursive command explosion')
            index = step['object']
            if type(index) is not int or index not in alive or handles[index] is None:
                raise ValueError('invalid live block handle')
            name = handles[index]
            outputs = leaves(name, []) if step.get('recursive', False) else definitions[name][:]
            alive.remove(index)
        elif action == 'group':
            picks = step.get('objects')
            if (set(step) != set(('action','objects')) or not isinstance(picks,list) or not picks or
                    any(type(i) is not int or i not in alive for i in picks) or len(set(picks)) != len(picks)):
                raise ValueError('invalid live group handles')
            outputs = []
        elif action == 'explode_batch':
            picks=step.get('objects')
            if (set(step)-set(('action','objects','group_output')) or not isinstance(picks,list) or not picks or
                    any(type(i) is not int or i not in alive or handles[i] is None for i in picks) or
                    len(set(picks))!=len(picks) or type(step.get('group_output',False)) is not bool):
                raise ValueError('invalid live batch block handles')
            outputs=[]
            for index in sorted(picks): outputs.extend(leaves(handles[index],[]))
            alive.difference_update(picks)
        elif action=='rename_definition':
            if set(step)!=set(('action','name','new_name')): raise ValueError('invalid block rename fields')
            old,new=_name(step['name']),_name(step['new_name'])
            if old not in definitions or new in definitions and new!=old: raise ValueError('invalid block rename names')
            members=definitions.pop(old);definitions[new]=members
            for key in definitions: definitions[key]=[new if v==old else v for v in definitions[key]]
            handles=[new if v==old else v for v in handles];outputs=[]
        elif action=='make_unique':
            picks=step.get('objects');new=_name(step.get('name'))
            if (set(step)!=set(('action','objects','name')) or not isinstance(picks,list) or not picks or
                    any(type(i) is not int or i not in alive or handles[i] is None for i in picks) or len(set(picks))!=len(picks)
                    or len(set(handles[i] for i in picks))!=1 or new in definitions or not re.match(r'^[A-Za-z0-9_-]+$',step['name'])):
                raise ValueError('invalid unique block sources or name')
            definitions[new]=definitions[handles[picks[0]]][:]
            for i in picks:handles[i]=new
            outputs=[]
        elif action=='duplicate_definition':
            if set(step)!=set(('action','name','new_name')):raise ValueError('invalid duplicate block fields')
            old,new=_name(step['name']),_name(step['new_name'])
            if old not in definitions or new in definitions or not re.match(r'^[A-Za-z0-9_-]+$',step['new_name']):raise ValueError('invalid duplicate block names')
            definitions[new]=definitions[old][:];outputs=[]
        elif action=='replace_block':
            picks=step.get('objects');target=_name(step.get('name'));replacement=step.get('replacement_instance')
            if (set(step)-set(('action','objects','name','all_instances','replacement_instance')) or not isinstance(picks,list) or not picks
                    or any(type(i)is not int or i not in alive or i in protected or handles[i]is None for i in picks) or len(set(picks))!=len(picks)
                    or len(set(handles[i]for i in picks))!=1 or target not in definitions
                    or (replacement is None and not re.match(r'^[A-Za-z0-9_-]+$',step['name']))
                    or (replacement is not None and (type(replacement)is not int or replacement not in alive or replacement in protected or handles[replacement]!=target))
                    or type(step.get('all_instances',False))is not bool):raise ValueError('invalid replacement block sources or name')
            original=handles[picks[0]]
            changed=[i for i in alive if handles[i]==original] if step.get('all_instances',False) else picks
            for i in changed:handles[i]=target
            outputs=[]
        elif action=='object_state':
            picks=step.get('objects')
            if set(step)!=set(('action','objects','mode')) or not isinstance(picks,list)or not picks or any(type(i)is not int or i not in alive for i in picks)or len(set(picks))!=len(picks)or step['mode']not in ('normal','hidden','locked'):raise ValueError('invalid object state')
            if step['mode']=='normal':protected.difference_update(picks)
            else:protected.update(picks)
            outputs=[]
        elif action=='delete_definition':
            if set(step)-set(('action','name','expect_failure')) or 'name' not in step or type(step.get('expect_failure',False)) is not bool: raise ValueError('invalid block delete fields')
            name=_name(step['name'])
            if name not in definitions: raise ValueError('undefined block deletion')
            referenced=any(name in members for members in definitions.values())
            if referenced!=step.get('expect_failure',False): raise ValueError('block deletion expectation does not match graph')
            if not referenced:
                del definitions[name]
                alive=set(i for i in alive if handles[i]!=name)
            outputs=[]
        else:
            raise ValueError('unknown block workflow action')
        alive.update(range(len(handles), len(handles) + len(outputs)))
        handles.extend(outputs)
        if len(handles) > MAX_HANDLES:
            raise ValueError('block workflow handle budget exceeded')
        records += record_cost()
        if records > MAX_RECORDS:
            raise ValueError('block recording budget exceeded')


def run(operation, tolerance, host):
    validate(operation)
    Rhino, System = host['Rhino'], host['System']
    import rhinoscriptsyntax as rs
    document = Rhino.RhinoDoc.ActiveDoc
    owned, owned_attributes, handles, layers, definitions = [], [], [], [], []
    definition_names, by_name = {}, {}
    prefix = 'VibocerosOracleBlock_' + str(System.Guid.NewGuid()) + '_'
    current_before = document.Layers.CurrentLayerIndex
    selected_before = [o.Id for o in document.Objects.GetSelectedObjects(False, False)]
    records_left = [MAX_RECORDS]
    group_start = document.Groups.Count
    record_groups = operation.get('record_groups', False)
    marker_ids=set()
    marker_handle_count=[0]

    def group_record(attributes):
        return [int(i) - group_start for i in (attributes.GetGroupList() or [])]

    def spend():
        records_left[0] -= 1
        if records_left[0] < 0:
            raise ValueError('block recording budget exceeded')

    def strings(owner):
        pairs = owner.GetUserStrings()
        return dict((str(k), str(pairs[k])) for k in pairs.AllKeys) if pairs else {}

    def attributes(spec, layer, name=None):
        a = Rhino.DocObjects.ObjectAttributes()
        owned_attributes.append(a)
        a.LayerIndex = layer
        value = spec.get('name') if spec.get('name') is not None else name
        a.Name = value.strip() if value is not None else ''
        a.ObjectColor = System.Drawing.Color.FromArgb(*spec.get('color', [0, 0, 0]))
        a.ColorSource = {'layer': Rhino.DocObjects.ObjectColorSource.ColorFromLayer,
                         'object': Rhino.DocObjects.ObjectColorSource.ColorFromObject,
                         'parent': Rhino.DocObjects.ObjectColorSource.ColorFromParent}[spec.get('color_source', 'layer')]
        for k, v in spec.get('user_text', {}).items():
            a.SetUserString(k, v)
        return a

    def attribute_record(a, geometry):
        color = a.ObjectColor
        # Native Block inserts this hidden bookkeeping UUID. Only suppress a
        # marker referencing one of this probe's source handles; arbitrary
        # caller text (including other dollar-prefixed keys) stays visible.
        user_text=strings(a)
        if '$block-instance-original-object-id$' in user_text:
            marker_ids.update(str(key).lower() for key in handles[marker_handle_count[0]:])
            marker_handle_count[0]=len(handles)
            user_text=_user_text_record(user_text,marker_ids)
        return dict(name=str(a.Name) if a.Name else None, layer='Source' if a.LayerIndex == layers[0] else 'Current',
                    color=[int(color.R), int(color.G), int(color.B)], color_source={'ColorFromLayer': 'layer', 'ColorFromObject': 'object', 'ColorFromParent': 'parent', 'ColorFromMaterial': 'material'}[str(a.ColorSource)],
                    user_text=user_text, geometry_user_text=strings(geometry))

    def transform_record(t):
        return [[float(t[r, c]) for c in range(4)] for r in range(4)]

    def reference_record(geometry):
        return dict(kind='block', definition=definition_names[geometry.ParentIdefId], transform=transform_record(geometry.Xform))

    def geometry_record(geometry, expanded=True):
        spend()
        if isinstance(geometry, Rhino.Geometry.InstanceReferenceGeometry):
            value = reference_record(geometry)
            if expanded:
                value['leaves'] = placed_leaves(geometry.ParentIdefId, geometry.Xform, [], [])
            return value
        domain = None
        if isinstance(geometry, Rhino.Geometry.Point):
            kind, points = 'point', [host['_xyz'](geometry.Location)]
        elif isinstance(geometry, Rhino.Geometry.PointCloud):
            kind, points = 'point_cloud', [host['_xyz'](p) for p in geometry.GetPoints()]
        elif isinstance(geometry, Rhino.Geometry.Mesh):
            kind, points = 'mesh', [host['_xyz'](p) for p in geometry.Vertices]
        elif isinstance(geometry, Rhino.Geometry.Brep) and geometry.IsSurface:
            kind = 'surface'
            domain, points = host['_plane_array_geometry_record'](geometry.Faces[0].UnderlyingSurface(), True)
        elif isinstance(geometry, Rhino.Geometry.Brep):
            kind = 'brep'
            domain, points = host['_plane_array_brep_record'](geometry)
        else:
            kind = 'surface' if isinstance(geometry, Rhino.Geometry.Surface) else 'curve'
            domain, points = host['_plane_array_geometry_record'](geometry, kind == 'surface')
        return dict(kind=kind, domain=domain, points=points)

    def placed_leaves(identifier, transform, path, stack):
        if identifier in stack or len(stack) >= 64:
            raise ValueError('invalid native block graph')
        definition = document.InstanceDefinitions.FindId(identifier)
        result = []
        for index, member in enumerate(definition.GetObjects()):
            geometry = member.Geometry
            member_path = path + [[definition_names[identifier], index]]
            if isinstance(geometry, Rhino.Geometry.InstanceReferenceGeometry):
                result.extend(placed_leaves(geometry.ParentIdefId, transform * geometry.Xform, member_path, stack + [identifier]))
            else:
                # ArcCurve.Transform cannot represent a general affine conic.
                # A block's placed locus is the affine image of its definition;
                # use its exact rational representation for this witness.
                placed = (geometry.ToNurbsCurve() if isinstance(geometry, Rhino.Geometry.ArcCurve)
                          and transform.SimilarityType == Rhino.Geometry.TransformSimilarityType.NotSimilarity
                          else geometry.Duplicate())
                try:
                    if not placed.Transform(transform):
                        raise ValueError('native block member transform failed')
                    value = dict(path=member_path, attributes=attribute_record(member.Attributes, geometry), geometry=geometry_record(placed))
                    if record_groups: value['groups'] = group_record(member.Attributes)
                    result.append(value)
                finally:
                    placed.Dispose()
        return result

    def snapshot():
        objects = []
        for index, identifier in enumerate(handles):
            obj = document.Objects.FindId(identifier)
            if obj is not None and not obj.IsDeleted:
                value = dict(handle=index, attributes=attribute_record(obj.Attributes, obj.Geometry), geometry=geometry_record(obj.Geometry))
                if record_groups: value['groups'] = group_record(obj.Attributes)
                objects.append(value)
        catalog = []
        for index in definitions:
            definition = document.InstanceDefinitions[index]
            if definition.IsDeleted: continue
            members=[]
            for m in definition.GetObjects():
                value=dict(attributes=attribute_record(m.Attributes,m.Geometry),geometry=geometry_record(m.Geometry,False))
                if record_groups: value['groups']=group_record(m.Attributes)
                members.append(value)
            value=dict(name=definition_names[definition.Id],members=members)
            if operation.get('record_management',False):
                top=len(definition.GetReferences(0));total=int(definition.UseCount())
                value['usage']=dict(top_level=top,nested=total-top,total=total,definition_references=live_definition_references(definition.Id))
            catalog.append(value)
        catalog.sort(key=lambda d: d['name'])
        value=dict(objects=objects, definitions=catalog)
        if operation.get('record_states',False):
            value['object_states']=[dict(handle=i,visible=bool(o.Attributes.Visible),locked=bool(o.IsLocked)) for i,key in enumerate(handles) for o in [document.Objects.FindId(key)] if o is not None and not o.IsDeleted]
        if record_groups:
            value['groups']=[dict(index=i-group_start,objects=[j for j,key in enumerate(handles) if document.Objects.FindId(key) is not None and not document.Objects.FindId(key).IsDeleted and i in (document.Objects.FindId(key).Attributes.GetGroupList() or [])]) for i in range(group_start,document.Groups.Count) if not document.Groups.IsDeleted(i)]
        return value

    def live_definition_references(identifier):
        # GetReferences(2) can retain objects from deleted parent definitions.
        # The manager's guard concerns the active catalog, inspected directly.
        return sum(1 for index in definitions if not document.InstanceDefinitions[index].IsDeleted
                   for obj in document.InstanceDefinitions[index].GetObjects()
                   if isinstance(obj.Geometry,Rhino.Geometry.InstanceReferenceGeometry) and obj.Geometry.ParentIdefId==identifier)

    def add_instance(index, transform, spec):
        a = attributes(spec, layers[1])
        identifier = document.Objects.AddInstanceObject(index, transform, a)
        if identifier == System.Guid.Empty:
            raise ValueError('native block insertion failed')
        handles.append(identifier)
        for k, v in spec.get('geometry_user_text', {}).items():
            if not rs.SetUserText(identifier, k, v, True):
                raise ValueError('native block geometry text failed')
        return identifier

    try:
        # Build every detached source before editing the document.
        for source in operation['sources']:
            geometry = host['_object_source'](source, tolerance)
            if geometry is None:
                raise ValueError('invalid native block source')
            owned.append(geometry)
        for label in ('Source', 'Current'):
            layer = Rhino.DocObjects.Layer()
            try:
                layer.Name = prefix + label
                layer.Color = System.Drawing.Color.Black
                index = document.Layers.Add(layer)
                if index < 0:
                    raise ValueError('native block layer failed')
                layers.append(index)
            finally:
                layer.Dispose()
        document.Layers.SetCurrentLayerIndex(layers[1], True)
        for index, geometry in enumerate(owned):
            spec = operation.get('attributes', [])[index] if operation.get('attributes') else {}
            for k, v in spec.get('geometry_user_text', {}).items():
                geometry.SetUserString(k, v)
            identifier = document.Objects.Add(geometry, attributes(spec, layers[0], 'source-' + str(index)))
            if identifier == System.Guid.Empty:
                raise ValueError('native block source insertion failed')
            handles.append(identifier)
            # Rhino wraps surfaces in a single-face Brep when adding them.
            # Attach geometry text to the admitted native object, not the
            # detached surface that was consumed by that wrapper.
            for k, v in spec.get('geometry_user_text', {}).items():
                if not rs.SetUserText(identifier, k, v, True):
                    raise ValueError('native block source geometry text failed')
        states = [snapshot()]
        for step in operation['steps']:
            action = step['action']
            start = len(handles)
            succeeded=None
            if action == 'create':
                if step.get('api','sdk')=='command':
                    settings=Rhino.DocObjects.ObjectEnumeratorSettings()
                    settings.NormalObjects=settings.LockedObjects=settings.HiddenObjects=True
                    before=set(o.Id for o in document.Objects.GetObjectList(settings))
                    document.Objects.UnselectAll()
                    for j in sorted(step['sources']): document.Objects.Select(handles[j])
                    # The scripted Block command asks for description, URL
                    # description and URL after its name, even when all are empty.
                    script='_-Block %s %s _Enter _Enter _Enter' % (','.join(str(v) for v in step['base']),prefix+step['name'])
                    if not rs.Command(script,False): raise ValueError('native Block command failed: '+str(Rhino.RhinoApp.CommandHistoryWindowText)[-1000:])
                    roots=[o for o in document.Objects.GetObjectList(settings) if o.Id not in before and isinstance(o.Geometry,Rhino.Geometry.InstanceReferenceGeometry)]
                    if len(roots)!=1: raise ValueError('native Block command did not return exactly one root')
                    definition=document.InstanceDefinitions.FindId(roots[0].Geometry.ParentIdefId)
                    definitions.append(definition.Index);by_name[_fold(step['name'])]=definition.Index
                    definition_names[definition.Id]=step['name'];handles.append(roots[0].Id)
                    value=snapshot();value['outputs']=[start];states.append(value)
                    continue
                picks = [document.Objects.FindId(handles[i]) for i in sorted(step['sources'])]
                geometry, attrs = [], []
                translation = Rhino.Geometry.Transform.Translation(-step['base'][0], -step['base'][1], -step['base'][2])
                for obj in picks:
                    g, a = obj.Geometry.Duplicate(), obj.Attributes.Duplicate()
                    owned.append(g)
                    owned_attributes.append(a)
                    if not g.Transform(translation):
                        raise ValueError('native block normalization failed')
                    geometry.append(g)
                    attrs.append(a)
                name = _fold(step['name'])
                gs = System.Array[Rhino.Geometry.GeometryBase](geometry)
                ats = System.Array[Rhino.DocObjects.ObjectAttributes](attrs)
                if name in by_name:
                    index = by_name[name]
                    if not document.InstanceDefinitions.ModifyGeometry(index, gs, ats):
                        raise ValueError('native block redefinition failed')
                else:
                    index = document.InstanceDefinitions.Add(prefix + step['name'], '', Rhino.Geometry.Point3d.Origin, gs, ats)
                    if index < 0:
                        raise ValueError('native block definition failed')
                    definitions.append(index)
                    by_name[name] = index
                    definition_names[document.InstanceDefinitions[index].Id] = step['name']
                add_instance(index, Rhino.Geometry.Transform.Translation(*step['base']), {})
                for obj in picks:
                    if not document.Objects.Delete(obj.Id, True):
                        raise ValueError('native block source deletion failed')
            elif action == 'insert':
                transform = Rhino.Geometry.Transform.Identity
                for r in range(4):
                    for c in range(4):
                        transform[r, c] = step['transform'][r][c]
                add_instance(by_name[_fold(step['name'])], transform, step.get('attributes', {}))
            elif action == 'group':
                if document.Groups.Add(prefix+'Group_'+str(document.Groups.Count),System.Array[System.Guid]([handles[i] for i in step['objects']])) < 0:
                    raise ValueError('native block grouping failed')
            elif action=='rename_definition':
                index=by_name[_fold(step['name'])];definition=document.InstanceDefinitions[index]
                if not document.InstanceDefinitions.Modify(index,prefix+step['new_name'],'',True): raise ValueError('native block rename failed')
                del by_name[_fold(step['name'])];by_name[_fold(step['new_name'])]=index;definition_names[definition.Id]=step['new_name']
            elif action=='make_unique':
                document.Objects.UnselectAll()
                for j in sorted(step['objects']):document.Objects.Select(handles[j])
                before=set(definition_names)
                if not rs.Command('_-CreateUniqueBlock '+prefix+step['name'],False):
                    raise ValueError('native unique block failed: '+str(Rhino.RhinoApp.CommandHistoryWindowText)[-1000:])
                additions=[d for d in document.InstanceDefinitions if not d.IsDeleted and d.Id not in before and d.Name.startswith(prefix)]
                if len(additions)!=1:raise ValueError('native unique block did not create exactly one definition')
                definition=additions[0];definitions.append(definition.Index);by_name[_fold(step['name'])]=definition.Index;definition_names[definition.Id]=step['name']
            elif action=='duplicate_definition':
                # Exercise the same native duplication command on a temporary
                # insert, then retain just its unused definition.
                temp=document.Objects.AddInstanceObject(by_name[_fold(step['name'])],Rhino.Geometry.Transform.Identity)
                if temp==System.Guid.Empty:raise ValueError('native duplicate temporary insert failed')
                try:
                    document.Objects.UnselectAll();document.Objects.Select(temp);before=set(definition_names)
                    if not rs.Command('_-CreateUniqueBlock '+prefix+step['new_name'],False):raise ValueError('native definition duplication failed')
                    additions=[d for d in document.InstanceDefinitions if not d.IsDeleted and d.Id not in before and d.Name.startswith(prefix)]
                    if len(additions)!=1:raise ValueError('native duplicate catalog mismatch')
                    definition=additions[0];definitions.append(definition.Index);by_name[_fold(step['new_name'])]=definition.Index;definition_names[definition.Id]=step['new_name']
                finally:document.Objects.Delete(temp,True)
            elif action=='object_state':
                for j in step['objects']:
                    obj=document.Objects.FindId(handles[j]);a=obj.Attributes.Duplicate();owned_attributes.append(a)
                    a.Mode={'normal':Rhino.DocObjects.ObjectMode.Normal,'hidden':Rhino.DocObjects.ObjectMode.Hidden,'locked':Rhino.DocObjects.ObjectMode.Locked}[step['mode']]
                    if not document.Objects.ModifyAttributes(obj.Id,a,True):raise ValueError('native root state failed')
            elif action=='replace_block':
                picks=sorted(step['objects']);original=document.Objects.FindId(handles[picks[0]]).InstanceDefinition
                extra=len(original.GetReferences(0))>len(picks)
                document.Objects.UnselectAll()
                for j in picks:document.Objects.Select(handles[j])
                choice=('_All ' if step.get('all_instances',False) else '_None ') if extra else ''
                replacement=step.get('replacement_instance')
                target=('_BlockDefinitionName '+prefix+step['name']) if replacement is None else '_SelID '+str(handles[replacement])
                script='_-ReplaceBlock '+choice+target
                if not rs.Command(script,False):raise ValueError('native ReplaceBlock failed: '+str(Rhino.RhinoApp.CommandHistoryWindowText)[-1400:])
            elif action=='delete_definition':
                index=by_name[_fold(step['name'])]
                # BlockManager prohibits deleting a definition nested in any
                # other catalog entry. SDK Delete(True) can instead remove
                # nested references; keep this declared manager policy explicit.
                nested=live_definition_references(document.InstanceDefinitions[index].Id)>0
                succeeded=False if nested else bool(document.InstanceDefinitions.Delete(index,True,True))
                if succeeded==step.get('expect_failure',False): raise ValueError('native deletion expectation mismatch for '+step['name']+': nested='+str(nested)+', succeeded='+str(succeeded))
                if succeeded: del by_name[_fold(step['name'])]
            elif action == 'explode_batch':
                settings=Rhino.DocObjects.ObjectEnumeratorSettings()
                settings.NormalObjects=settings.LockedObjects=settings.HiddenObjects=True
                before=set(o.Id for o in document.Objects.GetObjectList(settings))
                document.Objects.UnselectAll()
                script='_ExplodeBlock _GroupOutput=%s %s _Enter' % ('Yes' if step.get('group_output',False) else 'No',' '.join('_SelID %s' % handles[j] for j in sorted(step['objects'])))
                if not rs.Command(script,False): raise ValueError('native batch explosion failed: '+str(Rhino.RhinoApp.CommandHistoryWindowText)[-1000:])
                outputs=[o.Id for o in sorted(document.Objects.GetObjectList(settings),key=lambda o:o.RuntimeSerialNumber) if o.Id not in before]
                if not outputs: raise ValueError('native batch explosion returned no objects')
                handles.extend(outputs)
            else:
                if step.get('api', 'command') == 'command':
                    settings = Rhino.DocObjects.ObjectEnumeratorSettings()
                    settings.NormalObjects = settings.LockedObjects = settings.HiddenObjects = True
                    before = set(o.Id for o in document.Objects.GetObjectList(settings))
                    document.Objects.UnselectAll()
                    # With preselection both commands execute immediately;
                    # trailing option/Enter tokens would start another command.
                    if step.get('recursive',False) and ('group_output' in step):
                        script = '_ExplodeBlock _GroupOutput=%s _SelID %s _Enter' % ('Yes' if step['group_output'] else 'No',handles[step['object']])
                    else:
                        document.Objects.Select(handles[step['object']])
                        script = '_ExplodeBlock' if step.get('recursive', False) else '_Explode'
                    if not rs.Command(script, False):
                        raise ValueError('native block explosion command failed: ' + str(Rhino.RhinoApp.CommandHistoryWindowText)[-800:])
                    outputs = [o.Id for o in sorted(document.Objects.GetObjectList(settings), key=lambda o: o.RuntimeSerialNumber) if o.Id not in before]
                else:
                    outputs = rs.ExplodeBlockInstance(handles[step['object']], step.get('recursive', False))
                if not outputs:
                    raise ValueError('native block explosion failed')
                handles.extend(outputs)
            value = snapshot()
            value['outputs'] = list(range(start, len(handles)))
            if succeeded is not None: value['succeeded']=succeeded
            states.append(value)
        return dict(states=states), 0
    finally:
        # Only this probe's private layers/definitions can be changed here.
        for identifier in handles:
            obj = document.Objects.FindId(identifier)
            if obj is not None and not obj.IsDeleted:
                document.Objects.Delete(identifier, True)
        for index in reversed(definitions):
            if not document.InstanceDefinitions[index].IsDeleted: document.InstanceDefinitions.Delete(index, True, True)
        for index in range(group_start,document.Groups.Count):
            if not document.Groups.IsDeleted(index): document.Groups.Delete(index)
        document.Layers.SetCurrentLayerIndex(current_before, True)
        for index in reversed(layers):
            document.Layers.Delete(index, True)
        document.Objects.UnselectAll()
        for identifier in selected_before:
            document.Objects.Select(identifier)
        for a in reversed(owned_attributes):
            a.Dispose()
        for geometry in reversed(owned):
            geometry.Dispose()

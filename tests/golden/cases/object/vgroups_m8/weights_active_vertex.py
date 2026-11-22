# WP 8.3.7: the Edit Mode weight tools of the active vertex (the last vertex of the selection
# history): `object.vertex_weight_paste` (one group to the other selected vertices),
# `vertex_weight_copy` (every group, `ToolSettings.vertex_group_subset` = ALL),
# `vertex_weight_delete`, `vertex_weight_normalize_active_vertex` (a locked group keeps its
# weight, as `vgroup_validmap_get` with the lock flags) and `vertex_weight_set_active`; the
# `Invalid vertex group index` / `Vertex group is locked` errors, CANCELLED without an active
# vertex (a face last in the history) and the polls (Edit Mode only). Weights are read with
# `VertexGroup.weight()` and through DISPLACE modifiers (A along X, B along Y, C along Z).
# golden: startup=factory tier=T1 compare=objects,evaluated:Cube
import bpy
bpy.data.objects['Cube'].vertex_groups.new(name='A')
bpy.data.objects['Cube'].vertex_groups.new(name='B')
bpy.data.objects['Cube'].vertex_groups.new(name='C')
bpy.data.objects['Cube'].vertex_groups['A'].add([0], 0.4, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['B'].add([0], 0.2, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['C'].add([0], 0.6, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['A'].add([1], 0.1, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['C'].add([2, 5], 0.3, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['B'].add([6], 0.7, 'REPLACE')
bpy.ops.object.vertex_weight_paste(weight_group=0)
bpy.ops.object.vertex_weight_copy()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.vertex_weight_paste(weight_group=0)
bpy.ops.object.vertex_weight_normalize_active_vertex()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 2, 3, 0])
bpy.ops.object.vertex_weight_paste()
bpy.ops.object.vertex_weight_paste(weight_group=7)
bpy.ops.object.vertex_weight_paste(weight_group=0)
bpy.data.objects['Cube'].vertex_groups['B'].lock_weight = True
bpy.ops.object.vertex_weight_paste(weight_group=1)
bpy.ops.object.vertex_weight_delete(weight_group=1)
bpy.ops.frap.select_elements(type='VERT', indices=[5, 7], action='ADD')
bpy.ops.frap.select_elements(type='VERT', indices=[0], action='ADD')
bpy.ops.object.vertex_weight_copy()
bpy.ops.object.vertex_weight_delete(weight_group=2)
bpy.ops.object.vertex_weight_normalize_active_vertex()
bpy.ops.frap.select_elements(type='VERT', indices=[6], action='ADD')
bpy.data.objects['Cube'].vertex_groups['B'].lock_weight = False
bpy.ops.object.vertex_weight_normalize_active_vertex()
bpy.ops.object.vertex_weight_delete(weight_group=0)
bpy.ops.object.vertex_weight_set_active(weight_group=2)
bpy.ops.object.vertex_weight_set_active()
bpy.data.objects['Light'].location = (bpy.data.objects['Cube'].vertex_groups.active_index, 0, 0)
bpy.ops.frap.select_elements(type='FACE', indices=[0], action='ADD')
bpy.ops.object.vertex_weight_normalize_active_vertex()
bpy.ops.object.vertex_weight_copy()
bpy.ops.object.vertex_weight_delete(weight_group=0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube'].vertex_groups['A'].weight(0), bpy.data.objects['Cube'].vertex_groups['B'].weight(0), bpy.data.objects['Cube'].vertex_groups['A'].weight(3))
bpy.data.objects['Camera'].scale = (bpy.data.objects['Cube'].vertex_groups['A'].weight(5), bpy.data.objects['Cube'].vertex_groups['C'].weight(1), bpy.data.objects['Cube'].vertex_groups['B'].weight(6))
bpy.data.objects['Cube'].vertex_groups['C'].weight(0)
bpy.data.objects['Cube'].vertex_groups['A'].weight(6)
bpy.data.objects['Cube'].vertex_groups['C'].weight(4)
bpy.data.objects['Cube'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['A'].vertex_group = 'A'
bpy.data.objects['Cube'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['B'].vertex_group = 'B'
bpy.data.objects['Cube'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['C'].vertex_group = 'C'
bpy.data.objects['Cube'].modifiers['C'].direction = 'Z'

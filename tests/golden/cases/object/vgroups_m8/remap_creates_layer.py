# WP 8.3.7: `object.vertex_group_move` and `vertex_group_sort` in Object Mode create the weight
# layer of a mesh whose groups have no weights yet (`BKE_object_defgroup_array_get` calls
# `deform_verts_for_write()`), so a later `vertex_group_invert` assigns 1.0 to every vertex of
# every group (`auto_assign`) and `vertex_group_mirror` mirrors its pairs. In Edit Mode a sort
# leaves a BMesh without the layer alone. Weights are read through DISPLACE modifiers.
# golden: startup=factory tier=T1 compare=objects,evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002
import bpy
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Cube'].vertex_groups.new(name='B')
bpy.data.objects['Cube'].vertex_groups.new(name='A')
bpy.ops.object.vertex_group_move(direction='UP')
bpy.ops.object.vertex_group_invert(group_select_mode='ALL')
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube'].vertex_groups['A'].index, bpy.data.objects['Cube'].vertex_groups['B'].weight(3), bpy.data.objects['Cube'].vertex_groups.active_index)
bpy.data.objects['Cube'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['A'].vertex_group = 'A'
bpy.data.objects['Cube'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['B'].vertex_group = 'B'
bpy.data.objects['Cube'].modifiers['B'].direction = 'Y'
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.001']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube.001'].select_set(True)
bpy.data.objects['Cube.001'].vertex_groups.new(name='b.R')
bpy.data.objects['Cube.001'].vertex_groups.new(name='a.L')
bpy.ops.object.vertex_group_sort(sort_type='NAME')
bpy.ops.object.vertex_group_mirror(all_groups=True)
bpy.ops.object.vertex_group_invert(group_select_mode='ALL')
bpy.data.objects['Camera'].scale = (bpy.data.objects['Cube.001'].vertex_groups['a.L'].index, bpy.data.objects['Cube.001'].vertex_groups['b.R'].weight(5), bpy.data.objects['Cube.001'].vertex_groups.active_index + 1)
bpy.data.objects['Cube.001'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['A'].vertex_group = 'a.L'
bpy.data.objects['Cube.001'].modifiers['A'].direction = 'X'
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.002']
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube.002'].select_set(True)
bpy.data.objects['Cube.002'].vertex_groups.new(name='Q')
bpy.data.objects['Cube.002'].vertex_groups.new(name='P')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.vertex_group_sort(sort_type='NAME')
bpy.ops.object.vertex_group_move(direction='DOWN')
bpy.ops.object.vertex_group_invert(group_select_mode='ALL')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Light'].location = (bpy.data.objects['Cube.002'].vertex_groups['P'].index, bpy.data.objects['Cube.002'].vertex_groups.active_index, 0)
bpy.data.objects['Cube.002'].vertex_groups['P'].weight(0)
bpy.data.objects['Cube.002'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube.002'].modifiers['A'].vertex_group = 'P'
bpy.data.objects['Cube.002'].modifiers['A'].direction = 'X'

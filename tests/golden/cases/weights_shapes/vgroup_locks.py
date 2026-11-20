# WP 6.3: `VertexGroup.lock_weight` (`DG_LOCK_WEIGHT`). A locked group survives
# `vertex_group_remove(all_unlocked=True)`, keeps its weights in `vertex_group_normalize_all`,
# and makes `vertex_group_assign` / `vertex_group_remove_from` fail their poll while active.
# Every group locked: `normalize_all` reports `All groups are locked`. Also
# `vertex_groups.active_index` stores a negative index as given.
# golden: startup=factory tier=T1 compare=objects
import bpy
bpy.data.objects['Cube'].vertex_groups.new(name='A')
bpy.data.objects['Cube'].vertex_groups.new(name='B')
bpy.data.objects['Cube'].vertex_groups.new(name='C')
bpy.data.objects['Cube'].vertex_groups['A'].add([0, 1], 0.5, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['B'].add([0, 1], 0.25, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['C'].add([0], 0.25, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['A'].lock_weight = True
bpy.data.objects['Camera'].scale = (bpy.data.objects['Cube'].vertex_groups['A'].lock_weight, bpy.data.objects['Cube'].vertex_groups['B'].lock_weight, 1)
bpy.ops.object.vertex_group_normalize_all(lock_active=False)
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube'].vertex_groups['A'].weight(0), bpy.data.objects['Cube'].vertex_groups['B'].weight(0), bpy.data.objects['Cube'].vertex_groups['C'].weight(0))
bpy.data.objects['Camera'].rotation_euler = (bpy.data.objects['Cube'].vertex_groups['A'].weight(1), bpy.data.objects['Cube'].vertex_groups['B'].weight(1), 0)
bpy.ops.object.vertex_group_set_active(group='A')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.object.vertex_group_assign()
bpy.ops.object.vertex_group_remove_from()
bpy.ops.object.vertex_group_set_active(group='B')
bpy.ops.object.vertex_group_assign()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].vertex_groups['B'].lock_weight = True
bpy.data.objects['Cube'].vertex_groups['C'].lock_weight = True
bpy.ops.object.vertex_group_normalize_all(lock_active=False)
bpy.data.objects['Cube'].vertex_groups['B'].lock_weight = False
bpy.data.objects['Cube'].vertex_groups['C'].name = 'D'
bpy.data.objects['Light'].location = (bpy.data.objects['Cube'].vertex_groups['D'].lock_weight, bpy.data.objects['Cube'].vertex_groups['B'].weight(5), 0)
bpy.ops.object.vertex_group_remove(all_unlocked=True)
bpy.data.objects['Light'].scale = (bpy.data.objects['Cube'].vertex_groups[1].index + 1, bpy.data.objects['Cube'].vertex_groups[0].lock_weight, bpy.data.objects['Cube'].vertex_groups[1].lock_weight)
bpy.data.objects['Cube'].vertex_groups[2]
bpy.data.objects['Cube'].vertex_groups['B']
bpy.data.objects['Cube'].vertex_groups['D'].name
bpy.data.objects['Cube'].vertex_groups.active_index = -3
bpy.data.objects['Light'].rotation_euler = (bpy.data.objects['Cube'].vertex_groups.active_index, 0, 0)
bpy.ops.object.vertex_group_remove(all=True)
bpy.data.objects['Light'].rotation_euler = (bpy.data.objects['Cube'].vertex_groups.active_index, 0, 0)

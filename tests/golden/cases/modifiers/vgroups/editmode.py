# WP 6.10: the weights of a mesh in Edit Mode (the BMesh deform layer) reach the modifiers:
# weights assigned in Edit Mode (`vertex_group_assign`) and others from Object Mode drive MASK
# (`use_smooth`), DISPLACE and SOLIDIFY's shell group (through a MASK) of objects evaluated while
# their mesh is in Edit Mode (both objects, multi-object Edit Mode).
# golden: startup=factory compare=evaluated:B,evaluated:A tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'B'
bpy.data.objects['B'].vertex_groups.new(name='Out')
bpy.data.objects['B'].modifiers.new(name='M', type='SOLIDIFY')
bpy.data.objects['B'].modifiers['M'].thickness = 0.2
bpy.data.objects['B'].modifiers['M'].shell_vertex_group = 'Out'
bpy.data.objects['B'].modifiers.new(name='K', type='MASK')
bpy.data.objects['B'].modifiers['K'].vertex_group = 'Out'
bpy.data.objects['B'].modifiers['K'].invert_vertex_group = True
bpy.data.objects['B'].modifiers['K'].show_in_editmode = True
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'A'
bpy.data.objects['A'].vertex_groups.new(name='G')
bpy.data.objects['A'].vertex_groups['G'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['A'].modifiers.new(name='M', type='MASK')
bpy.data.objects['A'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['A'].modifiers['M'].threshold = 0.45
bpy.data.objects['A'].modifiers['M'].use_smooth = True
bpy.data.objects['A'].modifiers['M'].show_in_editmode = True
bpy.data.objects['A'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['A'].modifiers['D'].vertex_group = 'G'
bpy.data.objects['A'].modifiers['D'].strength = 0.5
bpy.data.objects['A'].modifiers['D'].show_in_editmode = True
bpy.data.objects['B'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
#@frap bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 8, 9])
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); bm.verts.ensure_lookup_table(); [setattr(bm.verts[i], 'select', True) for i in (0, 1, 2, 8, 9)]; bm.select_flush_mode(); bmesh.update_edit_mesh(bpy.context.object.data)
bpy.context.scene.tool_settings.vertex_group_weight = 0.9
bpy.ops.object.vertex_group_assign()

# WP 2.11: grid edges including boundary edges (not manifold, so not beveled) and an
# interior line ending at the boundary, one segment then two.
# golden: startup=factory compare=objects,context,mesh:Grid,mesh:Grid.001 tier=T2 order=exact
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Grid'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Grid']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2, 5, 6, 9])
bpy.ops.mesh.bevel(offset=0.1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Grid.001']
bpy.data.objects['Grid'].select_set(False)
bpy.data.objects['Grid.001'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[2, 5, 6, 9, 13, 16])
bpy.ops.mesh.bevel(offset=0.1, segments=2)

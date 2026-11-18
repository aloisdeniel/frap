# WP 2.11: profile shapes on cube corners: square (1.0) with even and odd segment
# counts, square-in (0.0), concave (0.25).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001,mesh:Cube.002,mesh:Cube.003 tier=T2 order=exact
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(9, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=4, profile=1.0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.001']
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Cube.001'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=3, profile=0.0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.002']
bpy.data.objects['Cube.001'].select_set(False)
bpy.data.objects['Cube.002'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=4, profile=0.25)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.003']
bpy.data.objects['Cube.002'].select_set(False)
bpy.data.objects['Cube.003'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=3, profile=1.0)

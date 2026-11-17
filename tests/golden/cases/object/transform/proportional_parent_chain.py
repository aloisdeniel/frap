# WP 1.21 (T0), proportional editing with a parent chain Camera -> Light -> Cube (selected) ->
# Cube.001: the ancestors of the selection stay in place, the child moves with its parent and
# by its own factor.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.mesh.primitive_cube_add(location=(1.5, -1, 0))
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.objects['Camera'].location = (0, 3, 0)
bpy.data.objects['Light'].parent = bpy.data.objects['Camera']
bpy.data.objects['Light'].location = (1, 1, 1)
bpy.data.objects['Cube'].parent = bpy.data.objects['Light']
bpy.data.objects['Cube.002'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube.002'].select_set(False)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.transform.translate(value=(0, 0, 1), use_proportional_edit=True, proportional_edit_falloff='LINEAR', proportional_size=20)

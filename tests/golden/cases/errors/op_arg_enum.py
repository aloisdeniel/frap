# WP 1.26: enum values that are not items (code OP_ARG_ENUM, TypeError) list the valid items in
# Blender's order. `object.mode_set` has a dynamic item list that depends on the active object:
# a mesh offers six modes, a camera or a light only OBJECT (frap raises these from the operator,
# code OP_EXEC_ERROR, with the same class and message).
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.ops.object.select_all(action='FOO')
bpy.ops.object.select_all(action='select')
bpy.ops.mesh.primitive_cube_add(align='FOO')
bpy.ops.mesh.primitive_circle_add(fill_type='FOO')
bpy.ops.mesh.primitive_cone_add(end_fill_type='TRIANGLE')
bpy.ops.export_scene.gltf(export_format='FOO')
bpy.ops.object.mode_set(mode='POSE')
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.mode_set(mode='SCULPT')
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.object.mode_set(mode='VERTEX_PAINT')
bpy.ops.object.mode_set(mode='OBJECT')

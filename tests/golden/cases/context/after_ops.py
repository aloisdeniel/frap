# WP 1.26: the context follows operators: an added primitive becomes the only selected and the
# active object, `select_all` changes the selection but not the active object, `delete`
# clears both, and context paths then resolve against the new state.
# golden: startup=factory tier=T0 compare=context,objects
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(location=(0, 0, 3))
bpy.context.object.scale = (2, 2, 2)
bpy.context.selected_objects[0].location.x = 1
bpy.ops.object.select_all(action='SELECT')
bpy.context.active_object.location.y = 1
bpy.ops.object.select_all(action='INVERT')
bpy.context.selected_objects[1]
bpy.ops.object.select_all(action='SELECT')
bpy.data.objects['Sphere'].select_set(False)
bpy.ops.object.delete()
bpy.context.object.name
bpy.context.selected_objects[0]
bpy.ops.mesh.primitive_cone_add()
bpy.context.object.location.z = 5

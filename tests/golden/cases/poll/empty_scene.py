# WP 1.26: polls in an empty startup and after deleting everything: primitives, `select_all`,
# `delete` and the glTF exporter still poll true (`select_all` passes through, `delete` is
# CANCELLED with nothing to delete), `mode_set` fails without an active object. Blender's
# values are asserted on `#@blender` lines; the calls check frap's (see poll/factory).
# golden: startup=empty tier=T0 compare=objects,context
import bpy
bpy.ops.mesh.primitive_plane_add.poll()
#@blender assert bpy.ops.mesh.primitive_plane_add.poll() is True
bpy.ops.object.select_all.poll()
#@blender assert bpy.ops.object.select_all.poll() is True
bpy.ops.object.delete.poll()
#@blender assert bpy.ops.object.delete.poll() is True
bpy.ops.object.mode_set.poll()
#@blender assert bpy.ops.object.mode_set.poll() is False
bpy.ops.export_scene.gltf.poll()
#@blender assert bpy.ops.export_scene.gltf.poll() is True
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
bpy.ops.mesh.primitive_plane_add()
bpy.ops.object.mode_set.poll()
#@blender assert bpy.ops.object.mode_set.poll() is True
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
bpy.ops.object.mode_set.poll()
#@blender assert bpy.ops.object.mode_set.poll() is False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.delete.poll()
#@blender assert bpy.ops.object.delete.poll() is True

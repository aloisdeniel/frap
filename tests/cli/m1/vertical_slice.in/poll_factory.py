# WP 1.26: `bpy.ops.<op>.poll()` in the factory scene (Object Mode, Cube active and selected).
# Line outcomes carry no values: a `#@blender` assert pins Blender's result (blessing fails if
# Blender disagrees), and tests/cli/m1/vertical_slice.trycmd runs a copy of this file through
# frap and pins frap's `"value"` of every poll() line (keep both copies identical).
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.ops.mesh.primitive_cube_add.poll()
#@blender assert bpy.ops.mesh.primitive_cube_add.poll() is True
bpy.ops.mesh.primitive_monkey_add.poll()
#@blender assert bpy.ops.mesh.primitive_monkey_add.poll() is True
bpy.ops.object.select_all.poll()
#@blender assert bpy.ops.object.select_all.poll() is True
bpy.ops.object.delete.poll()
#@blender assert bpy.ops.object.delete.poll() is True
bpy.ops.object.mode_set.poll()
#@blender assert bpy.ops.object.mode_set.poll() is True
bpy.ops.export_scene.gltf.poll()
#@blender assert bpy.ops.export_scene.gltf.poll() is True
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.mesh.primitive_monkey_add(location=(0, 0, 3))
bpy.ops.object.delete()

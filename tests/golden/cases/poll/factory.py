# WP 1.26: `bpy.ops.<op>.poll()` in the factory scene (Object Mode, Cube active and selected).
# Line outcomes do not carry values, so each poll is pinned twice: a `#@blender` assert states
# Blender's result (blessing fails if Blender disagrees), and the operator called right after
# succeeds on both sides exactly when its poll passes (a failing poll raises RuntimeError).
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

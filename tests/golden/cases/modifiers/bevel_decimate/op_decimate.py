# WP 4.5: mesh.decimate in Edit Mode: ratio 1 (the default) does nothing; everything selected
# collapses to the ratio (quads joined back); X symmetry.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Suzanne,mesh:Suzanne.001 tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.decimate()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_monkey_add(location=(0, 4, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.decimate(ratio=0.4)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_monkey_add(location=(4, 4, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.decimate(ratio=0.3, use_symmetry=True, symmetry_axis='X')

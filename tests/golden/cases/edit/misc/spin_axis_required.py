# WP 2.10: mesh.spin and mesh.screw need an explicit axis headless (Blender's invoke takes it
# from the view, exec does not): the default zero axis is "Invalid/unset axis" and nothing
# changes. mesh.screw also needs an open chain of selected vertices: a closed loop (the top
# face's edges) is "You have to select a string of connected vertices too".
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.spin()
bpy.ops.mesh.spin(steps=4, angle=1.0, center=(1, 0, 0))
bpy.ops.mesh.screw()
bpy.ops.mesh.screw(axis=(0, 0, 1))
bpy.data.meshes['Cube'].total_vert_sel

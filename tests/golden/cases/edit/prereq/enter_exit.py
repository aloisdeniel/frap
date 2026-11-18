# WP 2.2: entering and leaving Edit Mode keeps the mesh index for index (T1), and a deselect in
# Edit Mode is written back to the mesh on exit.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].vertices[0].select
bpy.context.mode

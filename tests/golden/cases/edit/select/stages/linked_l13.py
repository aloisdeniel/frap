# WP 2.4: edit/select/linked.py cut after its line 13,
#   bpy.ops.mesh.select_linked()
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.meshes['Cube'].edges[0].use_seam = True
bpy.data.meshes['Cube'].edges[3].use_seam = True
bpy.data.meshes['Cube'].edges[5].use_seam = True
bpy.data.meshes['Cube'].edges[10].use_seam = True
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_cube_add(size=1, location=(3, 0, 0))
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.select_linked()

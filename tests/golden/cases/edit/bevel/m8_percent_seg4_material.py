# WP 8.2: PERCENT offset with four segments and a material index for the new faces.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('B'))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset_type='PERCENT', offset_pct=20, segments=4, material=1)

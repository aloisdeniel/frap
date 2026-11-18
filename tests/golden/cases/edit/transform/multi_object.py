# WP 2.12: multi-object Edit Mode: one median pivot over both objects' selections (world
# space), each mesh moves in its own object space.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0), rotation=(0, 0, 0.5), scale=(1, 2, 1), size=1)
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.resize(value=(0.5, 0.5, 0.5))
bpy.ops.transform.rotate(value=0.3, orient_axis='Z')
bpy.ops.transform.translate(value=(0, 0, 1), orient_type='LOCAL')

# WP 2.12: transform.shrink_fatten moves vertices along their normals by an object-space
# distance; in face select mode the normals of the selected faces; use_even_offset applies the
# shell factor; proportional editing.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].scale = (2, 1, 1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1])
bpy.ops.transform.shrink_fatten(value=0.3)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 2])
bpy.ops.transform.shrink_fatten(value=0.25)
bpy.ops.transform.shrink_fatten(value=-0.2, use_even_offset=True)
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.transform.shrink_fatten(value=0.4, use_proportional_edit=True, proportional_size=3, proportional_edit_falloff='LINEAR')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.transform.shrink_fatten(value=0.3)

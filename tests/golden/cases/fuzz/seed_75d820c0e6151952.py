# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Torus tier=T2 tol=0.0001
# fuzz: seed=0x75d820c0e6151952 (replay)
import bpy
bpy.context.object.scale = (0.82, 1.26, 0.34)
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.geometry.color_attribute_add(name='Paint', domain='POINT')
bpy.ops.transform.shear(angle=-0.74, orient_axis_ortho='Y')
bpy.ops.object.modifier_copy(modifier='Subdivision')
bpy.ops.object.convert(target='MESH')
bpy.ops.object.parent_no_inverse_set()
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.mesh.primitive_torus_add(major_segments=10, minor_segments=5, minor_radius=0.31)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.vertex_group_copy()
bpy.ops.mesh.select_mode(use_extend=False, type='VERT')
bpy.ops.mesh.select_next_item()
bpy.ops.object.mode_set(mode='OBJECT')

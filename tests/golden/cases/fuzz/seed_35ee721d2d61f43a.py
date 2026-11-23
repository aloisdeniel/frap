# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Torus tier=T2 tol=0.0001
# fuzz: seed=0x35ee721d2d61f43a (replay)
import bpy
bpy.context.object.modifiers['Bevel'].segments = 4
bpy.context.object.modifiers['Triangulate'].quad_method = 'LONGEST_DIAGONAL'
bpy.ops.object.parent_inverse_apply()
bpy.ops.transform.resize(value=(0.65, 0.54, 1.1))
bpy.ops.mesh.primitive_torus_add(minor_segments=8, minor_radius=0.26)
bpy.ops.transform.rotate(value=-1.15, orient_axis='Z', proportional_size=0.5)
bpy.ops.mesh.uv_texture_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.objects['Camera'].select_set(False)
bpy.ops.transform.vert_crease(value=-0.47)
bpy.data.objects['Light'].select_set(False)
bpy.context.object.location.z = 0.93
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.shape_key_lock(action='LOCK')

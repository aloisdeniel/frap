# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=0.0001
# fuzz: seed=0x4079cd786f9d597f (replay)
import bpy
bpy.ops.object.shape_key_remove()
bpy.context.object.modifiers['EdgeSplit'].split_angle = 0.93
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.vertex_group_set_active(group='Group')
bpy.ops.mesh.primitive_monkey_add(location=(-0.56, -1.86, -0.73))
bpy.ops.uv.cube_project(cube_size=1.25, scale_to_bounds=True)
bpy.ops.uv.select_linked()
bpy.ops.object.material_slot_select()
bpy.ops.mesh.fill(use_beauty=True)
bpy.ops.mesh.dissolve_mode(use_verts=False, use_face_split=True)
bpy.ops.mesh.select_more(use_face_step=False)
bpy.ops.mesh.symmetry_snap(direction='POSITIVE_Z', factor=0.39)
bpy.context.object.vertex_groups.new(name='Group')
bpy.ops.transform.mirror(constraint_axis=(True, True, False), orient_type='GLOBAL')

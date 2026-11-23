# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=0.0001
# fuzz: seed=0x7958ba96648c1bf3 (replay)
import bpy
bpy.context.object.data.uv_layers.new(name='UV2')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.vertex_group_invert(auto_remove=True)
bpy.ops.mesh.fill(use_beauty=False)
bpy.ops.mesh.select_interior_faces()
bpy.ops.mesh.primitive_cone_add(vertices=6, radius1=0.77, depth=2.22, end_fill_type='NGON')
bpy.ops.object.material_slot_deselect()
bpy.ops.mesh.select_similar(type='FACE_AREA', compare='GREATER')
bpy.ops.mesh.set_sharpness_by_angle()
bpy.ops.uv.select_split()
bpy.ops.transform.vert_crease(value=0.05)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.object.modifiers['Triangulate'].quad_method = 'FIXED_ALTERNATE'
bpy.context.object.modifiers['Solidify'].thickness = 0.38

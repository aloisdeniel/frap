# golden: startup=factory compare=objects,context,mesh:Cone,mesh:Cube tier=T2 tol=0.0001
# fuzz: seed=0x4dbfbda07b96ca31 (replay)
import bpy
bpy.ops.object.shape_key_add(from_mix=True)
bpy.context.object.delta_location = (0.29, 0.91, 0.65)
bpy.ops.object.origin_set(type='ORIGIN_CURSOR')
bpy.ops.object.shade_smooth(keep_sharp_edges=True)
bpy.ops.mesh.primitive_cone_add(vertices=4, radius1=1.04, radius2=1.51, depth=2.77, end_fill_type='TRIFAN')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_edge_loop_multi()
bpy.ops.uv.select_split()
bpy.ops.object.vertex_group_select()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.visual_transform_apply()
bpy.context.object.data.uv_layers.new(name='UV2')
bpy.context.object.shape_key_add(name='Key', from_mix=True)
bpy.context.object.location.z = -0.36

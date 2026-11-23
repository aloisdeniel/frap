# golden: startup=factory compare=objects,context,mesh:Circle,mesh:Circle.001,mesh:Cube tier=T2 tol=0.0001
# fuzz: seed=0x8bb6d199698fd0ef (replay)
import bpy
bpy.ops.mesh.primitive_circle_add(radius=0.99, fill_type='TRIFAN')
bpy.context.object.data.vertices[1].select = False
bpy.ops.mesh.primitive_circle_add(radius=1.16)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.dissolve_edges(use_verts=True)
bpy.context.object.vertex_groups.new(name='Group')
bpy.context.scene.cursor.location = (-0.09, -1.0, -1.44)
bpy.ops.object.vertex_group_add()
bpy.ops.mesh.select_next_item()
bpy.data.objects['Circle'].select_set(False)
bpy.ops.mesh.select_by_pole_count(pole_count=6, extend=False, exclude_nonmanifold=True)
bpy.ops.mesh.vert_connect_nonplanar(angle_limit=0.95)
bpy.ops.object.vertex_group_assign()
bpy.ops.mesh.mark_sharp()

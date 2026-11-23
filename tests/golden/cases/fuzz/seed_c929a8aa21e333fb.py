# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=0.0001
# fuzz: seed=0xc929a8aa21e333fb (replay)
import bpy
bpy.ops.object.modifier_move_up(modifier='Subdivision')
bpy.context.object.modifiers['Triangulate'].quad_method = 'FIXED'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.extrude_vertices_move(TRANSFORM_OT_translate={'value': (0.5, 0, 0)})
bpy.ops.mesh.select_loose(extend=True)
bpy.ops.mesh.primitive_circle_add(vertices=10, radius=1.9)
bpy.ops.mesh.loop_to_region()
bpy.ops.mesh.extrude_edges_move(TRANSFORM_OT_translate={'value': (0, 0, 1)})
bpy.ops.mesh.normals_tools(mode='RESET')
bpy.data.objects['Cube'].select_set(False)
bpy.ops.mesh.tris_convert_to_quads(shape_threshold=2.52, seam=False, sharp=False, materials=False)
bpy.ops.mesh.extrude_repeat(steps=3, offset=(0.64, 0.13, -0.17))
bpy.context.object.location.z = -0.64
bpy.ops.uv.sphere_project(direction='ALIGN_TO_OBJECT', scale_to_bounds=True)

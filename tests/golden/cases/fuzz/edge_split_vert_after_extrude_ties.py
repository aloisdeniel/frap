# M8 stabilization round 3 (fuzz seed 0xd261cd89f7db8a29, run 0x8e417e57 trial 3719):
# `edge_split(type='EDGE')`, `extrude_context()`, `edge_split(type='VERT')` on the cube. The
# extrusion leaves its walls in Blender's pointer-hash order (D-65), so the walls of the
# vertex split take the coincident copies in another order: same mesh up to which coincident
# vertex each wall uses. This case writes Blender's extruded mesh (its element order and
# selection) first: on the same input frap splits exactly as Blender does.
# golden: startup=factory compare=objects,context,mesh:X tier=T1 tol=1e-5
import bpy
bpy.data.meshes.new('X').from_pydata([(1, 1, 1), (1, 1, -1), (1, -1, 1), (1, -1, -1), (-1, 1, 1), (-1, 1, -1), (-1, -1, 1), (-1, -1, -1), (-1, 1, -1), (-1, 1, -1), (-1, -1, -1), (-1, -1, -1), (1, 1, -1), (1, 1, -1), (1, 1, 1), (1, 1, 1), (-1, -1, 1), (-1, -1, 1), (1, -1, 1), (1, -1, 1), (1, -1, -1), (1, -1, -1), (-1, 1, 1), (-1, 1, 1), (1, 1, 1), (1, 1, -1), (1, -1, 1), (1, -1, -1), (-1, 1, 1), (-1, 1, -1), (-1, -1, 1), (-1, -1, -1), (-1, 1, -1), (-1, 1, -1), (-1, -1, -1), (-1, -1, -1), (1, 1, -1), (1, 1, -1), (1, 1, 1), (1, 1, 1), (-1, -1, 1), (-1, -1, 1), (1, -1, 1), (1, -1, 1), (1, -1, -1), (1, -1, -1), (-1, 1, 1), (-1, 1, 1)], [(8, 10), (12, 9), (14, 13), (11, 16), (18, 20), (22, 8), (19, 17), (15, 19), (11, 20), (17, 23), (23, 15), (21, 12), (9, 7), (1, 5), (4, 5), (10, 6), (7, 21), (0, 1), (3, 13), (14, 2), (4, 0), (18, 16), (6, 22), (2, 3), (32, 34), (36, 33), (38, 37), (35, 40), (42, 44), (46, 32), (43, 41), (39, 43), (35, 44), (41, 47), (47, 39), (45, 36), (33, 31), (25, 29), (28, 29), (34, 30), (31, 45), (24, 25), (27, 37), (38, 26), (28, 24), (42, 40), (30, 46), (26, 27), (21, 45), (36, 12), (18, 42), (44, 20), (2, 26), (38, 14), (9, 33), (31, 7), (8, 32), (46, 22), (0, 24), (28, 4), (5, 29), (25, 1), (19, 43), (41, 17), (16, 40), (15, 39), (30, 6), (10, 34), (35, 11), (3, 27), (47, 23), (37, 13)], [(15, 19, 17, 23), (20, 11, 16, 18), (10, 8, 22, 6), (9, 7, 21, 12), (13, 3, 2, 14), (5, 1, 0, 4), (39, 47, 41, 43), (44, 42, 40, 35), (34, 30, 46, 32), (33, 36, 45, 31), (37, 38, 26, 27), (29, 28, 24, 25), (12, 21, 45, 36), (20, 18, 42, 44), (14, 2, 26, 38), (7, 9, 33, 31), (22, 8, 32, 46), (4, 0, 24, 28), (1, 5, 29, 25), (17, 19, 43, 41), (18, 16, 40, 42), (5, 4, 28, 29), (19, 15, 39, 43), (6, 22, 46, 30), (8, 10, 34, 32), (10, 6, 30, 34), (11, 20, 44, 35), (2, 3, 27, 26), (9, 12, 36, 33), (21, 7, 31, 45), (23, 17, 41, 47), (13, 14, 38, 37), (0, 1, 25, 24), (15, 23, 47, 39), (16, 11, 35, 40), (3, 13, 37, 27)])
bpy.data.objects.new('X', bpy.data.meshes['X'])
bpy.context.scene.collection.objects.link(bpy.data.objects['X'])
bpy.data.objects['Cube'].select_set(False)
bpy.context.view_layer.objects.active = bpy.data.objects['X']
bpy.data.objects['X'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47])
bpy.ops.mesh.edge_split(type='VERT')

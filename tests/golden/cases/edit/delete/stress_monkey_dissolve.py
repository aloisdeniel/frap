# WP 2.7: dissolve_verts, dissolve_edges and dissolve_faces on scattered Suzanne selections
# (face splits, chains, joined regions), each followed by a delete that reuses the slots.
# golden: startup=empty compare=objects,context,mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[3, 14, 25, 36, 47, 58, 69, 80, 91, 102, 113, 124, 135, 146, 157, 168, 179, 190, 201, 212, 223, 234, 245, 256, 267, 278, 289, 300, 311, 322, 333, 344, 355, 366, 377, 388, 399, 410, 421, 432, 443, 454, 465, 476, 487, 498])
bpy.ops.mesh.dissolve_verts()
bpy.ops.frap.select_elements(type='VERT', indices=[1, 14, 27, 40, 53, 66, 79, 92, 105, 118, 131, 144, 157, 170, 183, 196, 209, 222, 235, 248, 261, 274, 287, 300, 313, 326, 339, 352, 365, 378, 391, 404, 417, 430])
bpy.ops.mesh.dissolve_verts(use_face_split=True)
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 9, 18, 27, 36, 45, 54, 63, 72, 81, 90, 99, 108, 117, 126, 135, 144, 153, 162, 171, 180, 189, 198, 207, 216, 225, 234, 243, 252, 261, 270, 279, 288, 297, 306, 315, 324, 333, 342, 351, 360, 369, 378, 387, 396, 405, 414, 423, 432, 441, 450, 459, 468, 477, 486, 495, 504, 513, 522, 531, 540, 549, 558, 567, 576, 585, 594, 603, 612, 621, 630, 639, 648, 657, 666, 675, 684, 693])
bpy.ops.mesh.dissolve_edges()
bpy.ops.frap.select_elements(type='EDGE', indices=[5, 22, 39, 56, 73, 90, 107, 124, 141, 158, 175, 192, 209, 226, 243, 260, 277, 294, 311, 328, 345, 362, 379, 396, 413, 430, 447, 464, 481, 498, 515, 532, 549, 566, 583])
bpy.ops.mesh.dissolve_edges(use_face_split=True, angle_threshold=0.4)
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 72, 76, 80, 84, 88, 92, 96, 100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144, 148, 152, 156, 160, 164, 168, 172, 176, 180, 184, 188, 192, 196, 200, 204, 208, 212, 216, 220, 224, 228, 232, 236, 240, 244, 248, 252, 256, 260, 264, 268, 272, 276, 280, 284, 288, 292, 296])
bpy.ops.mesh.dissolve_faces(use_verts=True)
bpy.ops.frap.select_elements(type='FACE', indices=[1, 7, 13, 19, 25, 31, 37, 43, 49, 55, 61, 67, 73, 79, 85, 91, 97, 103, 109, 115, 121, 127, 133, 139, 145, 151, 157, 163, 169, 175, 181, 187, 193, 199])
bpy.ops.mesh.delete(type='FACE')
bpy.ops.mesh.primitive_cube_add(size=0.3, location=(2, 0, 0))

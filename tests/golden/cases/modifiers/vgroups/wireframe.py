# WP 6.10: WIREFRAME with `vertex_group` (the thickness scaled by
# `thickness_vertex_group + weight * (1 - thickness_vertex_group)`), inverted, with unassigned
# vertices, even thickness and the original kept, and a missing group.
# golden: startup=factory compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'A'
bpy.data.objects['A'].vertex_groups.new(name='G')
bpy.data.objects['A'].vertex_groups['G'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['A'].modifiers.new(name='M', type='WIREFRAME')
bpy.data.objects['A'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['A'].modifiers['M'].thickness = 0.1
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'B'
bpy.data.objects['B'].vertex_groups.new(name='G')
bpy.data.objects['B'].vertex_groups['G'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['B'].modifiers.new(name='M', type='WIREFRAME')
bpy.data.objects['B'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['B'].modifiers['M'].thickness = 0.1
bpy.data.objects['B'].modifiers['M'].invert_vertex_group = True
bpy.data.objects['B'].modifiers['M'].thickness_vertex_group = 0.3
bpy.ops.mesh.primitive_monkey_add(size=2)
bpy.context.object.name = 'C'
bpy.data.objects['C'].vertex_groups.new(name='G')
bpy.data.objects['C'].vertex_groups['G'].add([5, 10, 20, 25, 35, 40, 50, 55, 65, 70, 80, 85, 95, 100, 110, 115, 125, 130, 140, 145, 155, 160, 170, 175, 185, 190, 200, 205, 215, 220, 230, 235, 245, 250, 260, 265, 275, 280, 290, 295, 305, 310, 320, 325, 335, 340, 350, 355, 365, 370, 380, 385, 395, 400, 410, 415, 425, 430, 440, 445, 455, 460, 470, 475, 485, 490, 500, 505], 0.25, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([8, 13, 23, 28, 38, 43, 53, 58, 68, 73, 83, 88, 98, 103, 113, 118, 128, 133, 143, 148, 158, 163, 173, 178, 188, 193, 203, 208, 218, 223, 233, 238, 248, 253, 263, 268, 278, 283, 293, 298, 308, 313, 323, 328, 338, 343, 353, 358, 368, 373, 383, 388, 398, 403, 413, 418, 428, 433, 443, 448, 458, 463, 473, 478, 488, 493, 503], 0.4, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([1, 11, 16, 26, 31, 41, 46, 56, 61, 71, 76, 86, 91, 101, 106, 116, 121, 131, 136, 146, 151, 161, 166, 176, 181, 191, 196, 206, 211, 221, 226, 236, 241, 251, 256, 266, 271, 281, 286, 296, 301, 311, 316, 326, 331, 341, 346, 356, 361, 371, 376, 386, 391, 401, 406, 416, 421, 431, 436, 446, 451, 461, 466, 476, 481, 491, 496, 506], 0.5, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([4, 14, 19, 29, 34, 44, 49, 59, 64, 74, 79, 89, 94, 104, 109, 119, 124, 134, 139, 149, 154, 164, 169, 179, 184, 194, 199, 209, 214, 224, 229, 239, 244, 254, 259, 269, 274, 284, 289, 299, 304, 314, 319, 329, 334, 344, 349, 359, 364, 374, 379, 389, 394, 404, 409, 419, 424, 434, 439, 449, 454, 464, 469, 479, 484, 494, 499], 0.6, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([2, 7, 17, 22, 32, 37, 47, 52, 62, 67, 77, 82, 92, 97, 107, 112, 122, 127, 137, 142, 152, 157, 167, 172, 182, 187, 197, 202, 212, 217, 227, 232, 242, 247, 257, 262, 272, 277, 287, 292, 302, 307, 317, 322, 332, 337, 347, 352, 362, 367, 377, 382, 392, 397, 407, 412, 422, 427, 437, 442, 452, 457, 467, 472, 482, 487, 497, 502], 0.75, 'REPLACE')
bpy.data.objects['C'].modifiers.new(name='M', type='WIREFRAME')
bpy.data.objects['C'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['C'].modifiers['M'].thickness = 0.1
bpy.data.objects['C'].modifiers['M'].use_even_offset = False
bpy.data.objects['C'].modifiers['M'].use_replace = False
bpy.ops.mesh.primitive_cube_add(size=2)
bpy.context.object.name = 'D'
bpy.data.objects['D'].vertex_groups.new(name='G')
bpy.data.objects['D'].vertex_groups['G'].add([4, 5, 6, 7], 1.0, 'REPLACE')
bpy.data.objects['D'].modifiers.new(name='M', type='WIREFRAME')
bpy.data.objects['D'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['D'].modifiers['M'].thickness = 0.1
bpy.data.objects['D'].modifiers['M'].thickness_vertex_group = 0.5
bpy.data.objects['D'].modifiers['M'].offset = 1.0
bpy.ops.mesh.primitive_cube_add(size=2)
bpy.context.object.name = 'E'
bpy.data.objects['E'].vertex_groups.new(name='H')
bpy.data.objects['E'].vertex_groups['H'].add([4, 5, 6, 7], 1.0, 'REPLACE')
bpy.data.objects['E'].modifiers.new(name='M', type='WIREFRAME')
bpy.data.objects['E'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['E'].modifiers['M'].thickness = 0.1

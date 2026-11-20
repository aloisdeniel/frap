# WP 6.10: MASK in VERTEX_GROUP mode with real weights: the threshold, `invert_vertex_group`,
# `use_smooth` (boundary faces cut at the threshold, inverted too), a missing group (unchanged), a
# group without weights (empty, unchanged when inverted), weights interpolated by SUBSURF before
# the mask, and the default threshold 0 with unassigned vertices.
# golden: startup=factory compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E,evaluated:F,evaluated:F2,evaluated:S,evaluated:Mo tier=T1
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
bpy.data.objects['A'].modifiers.new(name='M', type='MASK')
bpy.data.objects['A'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['A'].modifiers['M'].threshold = 0.4
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
bpy.data.objects['B'].modifiers.new(name='M', type='MASK')
bpy.data.objects['B'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['B'].modifiers['M'].threshold = 0.4
bpy.data.objects['B'].modifiers['M'].invert_vertex_group = True
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'C'
bpy.data.objects['C'].vertex_groups.new(name='G')
bpy.data.objects['C'].vertex_groups['G'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['C'].modifiers.new(name='M', type='MASK')
bpy.data.objects['C'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['C'].modifiers['M'].threshold = 0.45
bpy.data.objects['C'].modifiers['M'].use_smooth = True
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'D'
bpy.data.objects['D'].vertex_groups.new(name='G')
bpy.data.objects['D'].vertex_groups['G'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['D'].vertex_groups['G'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['D'].modifiers.new(name='M', type='MASK')
bpy.data.objects['D'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['D'].modifiers['M'].threshold = 0.45
bpy.data.objects['D'].modifiers['M'].use_smooth = True
bpy.data.objects['D'].modifiers['M'].invert_vertex_group = True
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'E'
bpy.data.objects['E'].vertex_groups.new(name='G')
bpy.data.objects['E'].vertex_groups['G'].add([0, 7, 14, 21, 28, 35], 0.0, 'REPLACE')
bpy.data.objects['E'].vertex_groups['G'].add([1, 8, 15, 22, 29, 36], 0.15, 'REPLACE')
bpy.data.objects['E'].vertex_groups['G'].add([2, 9, 16, 23, 30, 37], 0.35, 'REPLACE')
bpy.data.objects['E'].vertex_groups['G'].add([3, 10, 17, 24, 31, 38], 0.5, 'REPLACE')
bpy.data.objects['E'].vertex_groups['G'].add([4, 11, 18, 25, 32, 39], 0.65, 'REPLACE')
bpy.data.objects['E'].vertex_groups['G'].add([5, 12, 19, 26, 33, 40], 0.85, 'REPLACE')
bpy.data.objects['E'].vertex_groups['G'].add([6, 13, 20, 27, 34, 41], 1.0, 'REPLACE')
bpy.data.objects['E'].modifiers.new(name='M', type='MASK')
bpy.data.objects['E'].modifiers['M'].vertex_group = 'Nope'
bpy.data.objects['E'].modifiers['M'].threshold = 0.4
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'F'
bpy.data.objects['F'].vertex_groups.new(name='Other')
bpy.data.objects['F'].vertex_groups['Other'].add([4, 5, 6, 11, 12, 13, 18, 19, 20, 25, 26, 27, 32, 33, 34, 39, 40, 41], 1.0, 'REPLACE')
bpy.data.objects['F'].vertex_groups.new(name='Empty')
bpy.data.objects['F'].modifiers.new(name='M', type='MASK')
bpy.data.objects['F'].modifiers['M'].vertex_group = 'Empty'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)
bpy.context.object.name = 'F2'
bpy.data.objects['F2'].vertex_groups.new(name='Empty')
bpy.data.objects['F2'].modifiers.new(name='M', type='MASK')
bpy.data.objects['F2'].modifiers['M'].vertex_group = 'Empty'
bpy.data.objects['F2'].modifiers['M'].invert_vertex_group = True
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1)
bpy.context.object.name = 'S'
bpy.data.objects['S'].vertex_groups.new(name='G')
bpy.data.objects['S'].vertex_groups['G'].add([0], 0.0, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([12, 13, 16, 17, 19], 0.05, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([14, 15, 18, 20, 21], 0.25, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([1, 2, 3, 4, 5], 0.3, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([22, 23, 24, 25, 26, 27, 28, 29, 30, 31], 0.5, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([6, 7, 8, 9, 10], 0.7, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([32, 33, 34, 35, 36], 0.75, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([37, 38, 39, 40, 41], 0.95, 'REPLACE')
bpy.data.objects['S'].vertex_groups['G'].add([11], 1.0, 'REPLACE')
bpy.data.objects['S'].modifiers.new(name='Sub', type='SUBSURF')
bpy.data.objects['S'].modifiers['Sub'].levels = 1
bpy.data.objects['S'].modifiers.new(name='M', type='MASK')
bpy.data.objects['S'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['S'].modifiers['M'].threshold = 0.5
bpy.data.objects['S'].modifiers['M'].use_smooth = True
bpy.ops.mesh.primitive_monkey_add(size=2)
bpy.context.object.name = 'Mo'
bpy.data.objects['Mo'].vertex_groups.new(name='A')
bpy.data.objects['Mo'].vertex_groups['A'].add([0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42, 44, 46, 48, 50, 52, 54, 56, 58, 60, 62, 64, 78, 80, 82, 84, 86, 88, 91, 93, 95, 97, 99, 101, 103, 105, 107, 109, 111, 113, 115, 117, 119, 121, 123, 125, 127, 130, 132, 134, 139, 141, 143, 145, 147, 149, 151, 153, 155, 157, 159, 161, 163, 165, 167, 169, 171, 173, 178, 180, 182, 184, 186, 190, 192, 194, 196, 199, 201, 203, 205, 208, 210, 212, 214, 217, 219, 221, 224, 226, 228, 231, 233, 235, 237, 239, 241, 243, 245, 247, 249, 251, 253, 255, 257, 259, 261, 263, 265, 267, 269, 271, 273, 275, 277, 279, 281, 283, 285, 287, 289, 291, 293, 295, 297, 299, 309, 311, 313, 315, 317, 319, 321, 323, 325, 327, 329, 331, 333, 335, 337, 339, 341, 343, 345, 347, 349, 351, 353, 355, 357, 359, 361, 363, 365, 367, 369, 371, 373, 375, 377, 379, 381, 383, 385, 387, 389, 391, 393, 395, 397, 399, 401, 403, 405, 407, 409, 411, 413, 415, 417, 419, 421, 423, 425, 427, 429, 431, 433, 435, 437, 439, 441, 443, 445, 447, 449, 451, 453, 455, 457, 459, 461, 463, 465, 467, 469, 471, 473, 475, 477, 479, 481, 483, 485, 487, 489, 491, 493, 495, 497, 499, 501, 503, 505], 1.0, 'REPLACE')
bpy.data.objects['Mo'].vertex_groups.new(name='B')
bpy.data.objects['Mo'].vertex_groups['B'].add([5, 10, 20, 25, 35, 40, 50, 55, 65, 70, 80, 85, 95, 100, 110, 115, 125, 130, 140, 145, 155, 160, 170, 175, 185, 190, 200, 205, 215, 220, 230, 235, 245, 250, 260, 265, 275, 280, 290, 295, 305, 310, 320, 325, 335, 340, 350, 355, 365, 370, 380, 385, 395, 400, 410, 415, 425, 430, 440, 445, 455, 460, 470, 475, 485, 490, 500, 505], 0.25, 'REPLACE')
bpy.data.objects['Mo'].vertex_groups['B'].add([8, 13, 23, 28, 38, 43, 53, 58, 68, 73, 83, 88, 98, 103, 113, 118, 128, 133, 143, 148, 158, 163, 173, 178, 188, 193, 203, 208, 218, 223, 233, 238, 248, 253, 263, 268, 278, 283, 293, 298, 308, 313, 323, 328, 338, 343, 353, 358, 368, 373, 383, 388, 398, 403, 413, 418, 428, 433, 443, 448, 458, 463, 473, 478, 488, 493, 503], 0.4, 'REPLACE')
bpy.data.objects['Mo'].vertex_groups['B'].add([1, 11, 16, 26, 31, 41, 46, 56, 61, 71, 76, 86, 91, 101, 106, 116, 121, 131, 136, 146, 151, 161, 166, 176, 181, 191, 196, 206, 211, 221, 226, 236, 241, 251, 256, 266, 271, 281, 286, 296, 301, 311, 316, 326, 331, 341, 346, 356, 361, 371, 376, 386, 391, 401, 406, 416, 421, 431, 436, 446, 451, 461, 466, 476, 481, 491, 496, 506], 0.5, 'REPLACE')
bpy.data.objects['Mo'].vertex_groups['B'].add([4, 14, 19, 29, 34, 44, 49, 59, 64, 74, 79, 89, 94, 104, 109, 119, 124, 134, 139, 149, 154, 164, 169, 179, 184, 194, 199, 209, 214, 224, 229, 239, 244, 254, 259, 269, 274, 284, 289, 299, 304, 314, 319, 329, 334, 344, 349, 359, 364, 374, 379, 389, 394, 404, 409, 419, 424, 434, 439, 449, 454, 464, 469, 479, 484, 494, 499], 0.6, 'REPLACE')
bpy.data.objects['Mo'].vertex_groups['B'].add([2, 7, 17, 22, 32, 37, 47, 52, 62, 67, 77, 82, 92, 97, 107, 112, 122, 127, 137, 142, 152, 157, 167, 172, 182, 187, 197, 202, 212, 217, 227, 232, 242, 247, 257, 262, 272, 277, 287, 292, 302, 307, 317, 322, 332, 337, 347, 352, 362, 367, 377, 382, 392, 397, 407, 412, 422, 427, 437, 442, 452, 457, 467, 472, 482, 487, 497, 502], 0.75, 'REPLACE')
bpy.data.objects['Mo'].modifiers.new(name='M', type='MASK')
bpy.data.objects['Mo'].modifiers['M'].vertex_group = 'B'

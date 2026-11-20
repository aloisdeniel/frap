# WP 6.10: DECIMATE COLLAPSE weighted by `vertex_group` (`vertex_group_factor`), inverted, with
# symmetry, and with a missing group.
# golden: startup=factory compare=evaluated:A,evaluated:B,evaluated:C,evaluated:D,evaluated:E tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add(size=2)
bpy.context.object.name = 'A'
bpy.data.objects['A'].vertex_groups.new(name='G')
bpy.data.objects['A'].vertex_groups['G'].add([392, 394, 396, 398, 400, 402, 404, 406, 408, 410, 420, 422, 424, 426, 428, 464, 466, 468, 470, 472, 476, 482, 484, 486, 488, 490, 492, 494, 496, 498, 500, 502], 0.0, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([98, 310, 312, 388, 414, 416, 418, 450, 452, 456, 474, 478, 480, 506], 0.05, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([96, 294, 296, 314, 368, 370, 376, 382, 390, 412, 430, 432, 434, 436, 438, 440, 448, 454, 458, 460, 462, 504], 0.1, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([100, 150, 152, 154, 292, 318, 362, 442, 444, 446], 0.15, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([43, 45, 94, 120, 122, 124, 148, 242, 244, 246, 270, 298, 320, 322, 364, 366, 372, 374, 378], 0.2, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([3, 5, 39, 41, 47, 49, 102, 118, 248, 266, 268, 272, 290, 316, 354, 356, 358, 360, 380], 0.25, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([1, 7, 9, 11, 31, 33, 35, 37, 51, 53, 61, 63, 65, 83, 85, 92, 116, 126, 146, 156, 240, 250, 264, 274, 300, 384, 386], 0.3, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([13, 25, 55, 59, 81, 87, 104, 158, 170, 179, 209, 236, 238, 276, 278, 288, 324, 330, 334, 350, 352], 0.35, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([15, 17, 23, 27, 29, 57, 79, 89, 106, 110, 114, 128, 160, 166, 168, 172, 174, 181, 211, 213, 215, 232, 234, 252, 254, 256, 258, 260, 262, 280, 282, 286, 326, 328, 332, 340, 342, 344, 346, 348], 0.4, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([19, 21, 108, 112, 131, 133, 135, 140, 142, 144, 162, 164, 183, 185, 187, 191, 193, 195, 202, 204, 206, 220, 222, 225, 227, 284, 336, 338], 0.45, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 90, 129, 136, 137, 138, 175, 176, 177, 188, 189, 196, 197, 198, 199, 200, 207, 216, 217, 218, 223, 228, 229, 230, 301, 302, 303, 304, 305, 306, 307, 308], 0.5, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([18, 20, 107, 111, 130, 132, 134, 139, 141, 143, 161, 163, 182, 184, 186, 190, 192, 194, 201, 203, 205, 219, 221, 224, 226, 283, 335, 337], 0.55, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([14, 16, 22, 26, 28, 56, 78, 88, 105, 109, 113, 127, 159, 165, 167, 171, 173, 180, 210, 212, 214, 231, 233, 251, 253, 255, 257, 259, 261, 279, 281, 285, 325, 327, 331, 339, 341, 343, 345, 347], 0.6, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([12, 24, 54, 58, 80, 86, 103, 157, 169, 178, 208, 235, 237, 275, 277, 287, 323, 329, 333, 349, 351], 0.65, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([0, 6, 8, 10, 30, 32, 34, 36, 50, 52, 60, 62, 64, 82, 84, 91, 115, 125, 145, 155, 239, 249, 263, 273, 299, 383, 385], 0.7, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([2, 4, 38, 40, 46, 48, 101, 117, 247, 265, 267, 271, 289, 315, 353, 355, 357, 359, 379], 0.75, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([42, 44, 93, 119, 121, 123, 147, 241, 243, 245, 269, 297, 319, 321, 363, 365, 371, 373, 377], 0.8, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([99, 149, 151, 153, 291, 317, 361, 441, 443, 445], 0.85, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([95, 293, 295, 313, 367, 369, 375, 381, 389, 411, 429, 431, 433, 435, 437, 439, 447, 453, 457, 459, 461, 503], 0.9, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([97, 309, 311, 387, 413, 415, 417, 449, 451, 455, 473, 477, 479, 505], 0.95, 'REPLACE')
bpy.data.objects['A'].vertex_groups['G'].add([391, 393, 395, 397, 399, 401, 403, 405, 407, 409, 419, 421, 423, 425, 427, 463, 465, 467, 469, 471, 475, 481, 483, 485, 487, 489, 491, 493, 495, 497, 499, 501], 1.0, 'REPLACE')
bpy.data.objects['A'].modifiers.new(name='M', type='DECIMATE')
bpy.data.objects['A'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['A'].modifiers['M'].ratio = 0.4
bpy.ops.mesh.primitive_monkey_add(size=2)
bpy.context.object.name = 'B'
bpy.data.objects['B'].vertex_groups.new(name='G')
bpy.data.objects['B'].vertex_groups['G'].add([88, 89, 90, 305], 0.0, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([84, 85, 86, 87, 134, 135, 136, 171, 172, 173, 174, 175, 216, 217, 218, 228, 229, 230, 333, 334, 339, 340], 0.05, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([71, 129, 132, 133, 169, 170, 219, 220, 223, 224, 225, 226, 227, 306, 335, 336], 0.1, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([68, 82, 83, 130, 131, 167, 168, 184, 185, 221, 222, 329, 330], 0.15, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([307], 0.2, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([304, 308, 331, 332, 337, 338], 0.25, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([77, 80, 81, 165, 166, 186, 187, 188, 325, 326, 327, 328, 347, 348], 0.3, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([69, 143, 144, 189, 190, 191, 207, 208, 209, 212, 213, 341, 342, 349, 350], 0.35, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([70, 78, 79, 137, 141, 142, 192, 193, 198, 201, 202, 203, 204, 205, 206, 210, 211, 214, 215, 303, 343, 344, 345, 346, 383, 384, 385, 386], 0.4, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([91, 92, 138, 139, 140, 163, 164, 194, 195, 196, 197, 199, 200, 299, 300, 319, 320, 351, 352, 389, 390, 391, 392, 411, 412, 501, 502, 503, 504], 0.45, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([6, 7, 8, 9, 93, 94, 115, 116, 145, 146, 176, 297, 298, 317, 318, 321, 322, 379, 380, 409, 410, 417, 418, 419, 420, 437, 438, 439, 440, 441, 442, 443, 444, 445, 446, 453, 454, 455, 456, 471, 472, 473, 474], 0.5, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([2, 3, 4, 5, 10, 11, 14, 15, 16, 17, 62, 63, 76, 95, 96, 113, 114, 117, 118, 147, 148, 180, 181, 247, 248, 249, 250, 251, 252, 261, 262, 263, 264, 265, 266, 323, 324, 393, 394, 407, 408, 421, 422, 433, 434, 435, 436, 457, 458, 459, 460, 469, 470, 477, 478, 479, 480, 481, 482, 499, 500], 0.55, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([0, 1, 12, 13, 18, 19, 20, 21, 22, 23, 42, 43, 44, 45, 46, 47, 48, 49, 56, 57, 58, 59, 60, 61, 64, 65, 119, 120, 149, 150, 177, 182, 183, 231, 232, 233, 234, 245, 246, 267, 268, 279, 280, 281, 282, 295, 296, 309, 310, 423, 424, 431, 432, 447, 448, 449, 450, 451, 452, 461, 462, 467, 468, 483, 484, 485, 486, 487, 488, 489, 490], 0.6, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([24, 25, 36, 37, 50, 51, 54, 55, 111, 112, 121, 122, 243, 244, 253, 254, 259, 260, 269, 270, 271, 272, 311, 312, 313, 314, 377, 378, 381, 382, 395, 396, 405, 406, 415, 416, 427, 428, 429, 430, 463, 464, 465, 466, 475, 476, 491, 492, 497, 498], 0.65, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([26, 27, 28, 29, 34, 35, 38, 39, 40, 41, 52, 53, 66, 67, 72, 97, 98, 109, 110, 123, 124, 125, 126, 151, 152, 153, 154, 161, 162, 235, 236, 237, 238, 239, 240, 241, 242, 255, 256, 257, 258, 273, 274, 275, 276, 277, 278, 293, 294, 315, 316, 361, 362, 387, 388, 401, 402, 403, 404, 413, 414, 425, 426, 493, 494, 505, 506], 0.7, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([30, 31, 32, 33, 99, 100, 107, 108, 127, 128, 155, 156, 178, 179, 283, 284, 291, 292, 359, 360, 363, 364, 375, 376, 397, 398, 399, 400, 495, 496], 0.75, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([73, 75, 101, 102, 157, 158, 159, 160, 289, 290, 367, 368, 369, 370], 0.8, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([105, 106, 285, 286, 287, 288, 365, 366, 373, 374], 0.85, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([103, 104, 371, 372], 0.9, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([74, 301, 353, 354, 355, 356, 357, 358], 0.95, 'REPLACE')
bpy.data.objects['B'].vertex_groups['G'].add([302], 1.0, 'REPLACE')
bpy.data.objects['B'].modifiers.new(name='M', type='DECIMATE')
bpy.data.objects['B'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['B'].modifiers['M'].ratio = 0.4
bpy.data.objects['B'].modifiers['M'].invert_vertex_group = True
bpy.data.objects['B'].modifiers['M'].vertex_group_factor = 2.0
bpy.ops.mesh.primitive_monkey_add(size=2)
bpy.context.object.name = 'C'
bpy.data.objects['C'].vertex_groups.new(name='G')
bpy.data.objects['C'].vertex_groups['G'].add([5, 10, 20, 25, 35, 40, 50, 55, 65, 70, 80, 85, 95, 100, 110, 115, 125, 130, 140, 145, 155, 160, 170, 175, 185, 190, 200, 205, 215, 220, 230, 235, 245, 250, 260, 265, 275, 280, 290, 295, 305, 310, 320, 325, 335, 340, 350, 355, 365, 370, 380, 385, 395, 400, 410, 415, 425, 430, 440, 445, 455, 460, 470, 475, 485, 490, 500, 505], 0.25, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([8, 13, 23, 28, 38, 43, 53, 58, 68, 73, 83, 88, 98, 103, 113, 118, 128, 133, 143, 148, 158, 163, 173, 178, 188, 193, 203, 208, 218, 223, 233, 238, 248, 253, 263, 268, 278, 283, 293, 298, 308, 313, 323, 328, 338, 343, 353, 358, 368, 373, 383, 388, 398, 403, 413, 418, 428, 433, 443, 448, 458, 463, 473, 478, 488, 493, 503], 0.4, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([1, 11, 16, 26, 31, 41, 46, 56, 61, 71, 76, 86, 91, 101, 106, 116, 121, 131, 136, 146, 151, 161, 166, 176, 181, 191, 196, 206, 211, 221, 226, 236, 241, 251, 256, 266, 271, 281, 286, 296, 301, 311, 316, 326, 331, 341, 346, 356, 361, 371, 376, 386, 391, 401, 406, 416, 421, 431, 436, 446, 451, 461, 466, 476, 481, 491, 496, 506], 0.5, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([4, 14, 19, 29, 34, 44, 49, 59, 64, 74, 79, 89, 94, 104, 109, 119, 124, 134, 139, 149, 154, 164, 169, 179, 184, 194, 199, 209, 214, 224, 229, 239, 244, 254, 259, 269, 274, 284, 289, 299, 304, 314, 319, 329, 334, 344, 349, 359, 364, 374, 379, 389, 394, 404, 409, 419, 424, 434, 439, 449, 454, 464, 469, 479, 484, 494, 499], 0.6, 'REPLACE')
bpy.data.objects['C'].vertex_groups['G'].add([2, 7, 17, 22, 32, 37, 47, 52, 62, 67, 77, 82, 92, 97, 107, 112, 122, 127, 137, 142, 152, 157, 167, 172, 182, 187, 197, 202, 212, 217, 227, 232, 242, 247, 257, 262, 272, 277, 287, 292, 302, 307, 317, 322, 332, 337, 347, 352, 362, 367, 377, 382, 392, 397, 407, 412, 422, 427, 437, 442, 452, 457, 467, 472, 482, 487, 497, 502], 0.75, 'REPLACE')
bpy.data.objects['C'].modifiers.new(name='M', type='DECIMATE')
bpy.data.objects['C'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['C'].modifiers['M'].ratio = 0.4
bpy.data.objects['C'].modifiers['M'].use_symmetry = True
bpy.data.objects['C'].modifiers['M'].vertex_group_factor = 0.5
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1)
bpy.context.object.name = 'D'
bpy.data.objects['D'].vertex_groups.new(name='G')
bpy.data.objects['D'].vertex_groups['G'].add([1, 5, 6, 9, 10, 13, 14, 15, 16, 21, 22, 23, 25, 30, 32, 36, 37, 38, 41], 1.0, 'REPLACE')
bpy.data.objects['D'].modifiers.new(name='M', type='DECIMATE')
bpy.data.objects['D'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['D'].modifiers['M'].ratio = 0.4
bpy.data.objects['D'].modifiers['M'].use_collapse_triangulate = True
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1)
bpy.context.object.name = 'E'
bpy.data.objects['E'].vertex_groups.new(name='H')
bpy.data.objects['E'].vertex_groups['H'].add([1, 5, 6, 9, 10, 13, 14, 15, 16, 21, 22, 23, 25, 30, 32, 36, 37, 38, 41], 1.0, 'REPLACE')
bpy.data.objects['E'].modifiers.new(name='M', type='DECIMATE')
bpy.data.objects['E'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['E'].modifiers['M'].ratio = 0.4

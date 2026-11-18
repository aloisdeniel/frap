# WP 4.2: WELD (ALL, default 0.001) merges the four coincident vertices of two touching cubes;
# the two coincident faces become one (duplicate-face removal) and the shared edges merge.
# golden: startup=empty compare=evaluated:W tier=T1
import bpy
bpy.data.meshes.new('W')
bpy.data.meshes['W'].from_pydata([(-1, -1, -1), (-1, -1, 1), (-1, 1, -1), (-1, 1, 1), (1, -1, -1), (1, -1, 1), (1, 1, -1), (1, 1, 1), (1, -1, -1), (1, -1, 1), (1, 1, -1), (1, 1, 1), (3, -1, -1), (3, -1, 1), (3, 1, -1), (3, 1, 1)], [], [(0, 1, 3, 2), (2, 3, 7, 6), (6, 7, 5, 4), (4, 5, 1, 0), (2, 6, 4, 0), (7, 3, 1, 5), (8, 9, 11, 10), (10, 11, 15, 14), (14, 15, 13, 12), (12, 13, 9, 8), (10, 14, 12, 8), (15, 11, 9, 13)])
bpy.data.objects.new('W', bpy.data.meshes['W'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W'])
bpy.data.objects['W'].modifiers.new(name='Weld', type='WELD')

# WP 4.2: WELD (CONNECTED, loose_edges): only edges without faces are collapsed; the short edge
# of the triangle stays, the short loose edges merge.
# golden: startup=empty compare=evaluated:W tier=T1
import bpy
bpy.data.meshes.new('W')
bpy.data.meshes['W'].from_pydata([(0, 0, 0), (1, 0, 0), (0, 0.05, 0), (2, 0, 0), (2.05, 0, 0), (2.1, 0, 0), (3, 0, 0)], [(3, 4), (4, 5), (5, 6)], [(0, 1, 2)])
bpy.data.objects.new('W', bpy.data.meshes['W'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W'])
bpy.data.objects['W'].modifiers.new(name='Weld', type='WELD')
bpy.data.objects['W'].modifiers['Weld'].mode = 'CONNECTED'
bpy.data.objects['W'].modifiers['Weld'].merge_threshold = 0.1
bpy.data.objects['W'].modifiers['Weld'].loose_edges = True

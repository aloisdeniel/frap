# WP 4.2: SOLIDIFY "Complex" on non-manifold geometry: three quads sharing one edge (a fan, the
# faces sorted by angle around the edge), two triangles touching at a single vertex (a split
# vertex), a T-junction of a grid and an extruded edge; FIXED and EVEN thickness modes and the
# ROUND/FLAT boundary fixes.
# golden: startup=empty compare=evaluated:Fan,evaluated:Bow,evaluated:Fan.001,evaluated:Bow.001 tier=T1 order=canonical
import bpy
bpy.data.meshes.new('Fan')
bpy.data.meshes['Fan'].from_pydata([(0, 0, 0), (0, 0, 1), (1, 0, 0), (1, 0, 1), (-0.5, 0.8, 0), (-0.5, 0.8, 1), (-0.5, -0.8, 0), (-0.5, -0.8, 1)], [], [(0, 2, 3, 1), (0, 1, 5, 4), (0, 6, 7, 1)])
bpy.data.objects.new('Fan', bpy.data.meshes['Fan'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Fan'])
bpy.data.objects['Fan'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Fan'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Fan'].modifiers['Solidify'].thickness = 0.1
bpy.data.meshes.new('Bow')
bpy.data.meshes['Bow'].from_pydata([(0, 0, 0), (1, 0.5, 0), (1, -0.5, 0), (-1, 0.5, 0.2), (-1, -0.5, 0.2)], [], [(0, 2, 1), (0, 3, 4)])
bpy.data.objects.new('Bow', bpy.data.meshes['Bow'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Bow'])
bpy.data.objects['Bow'].location = (3, 0, 0)
bpy.data.objects['Bow'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Bow'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Bow'].modifiers['Solidify'].thickness = 0.1
bpy.data.objects['Bow'].modifiers['Solidify'].nonmanifold_boundary_mode = 'ROUND'
bpy.data.objects.new('Fan.001', bpy.data.meshes['Fan'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Fan.001'])
bpy.data.objects['Fan.001'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Fan.001'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Fan.001'].modifiers['Solidify'].thickness = -0.15
bpy.data.objects['Fan.001'].modifiers['Solidify'].nonmanifold_thickness_mode = 'EVEN'
bpy.data.objects['Fan.001'].modifiers['Solidify'].nonmanifold_boundary_mode = 'FLAT'
bpy.data.objects.new('Bow.001', bpy.data.meshes['Bow'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Bow.001'])
bpy.data.objects['Bow.001'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Bow.001'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Bow.001'].modifiers['Solidify'].thickness = 0.2
bpy.data.objects['Bow.001'].modifiers['Solidify'].offset = 0.5
bpy.data.objects['Bow.001'].modifiers['Solidify'].nonmanifold_thickness_mode = 'FIXED'
bpy.data.objects['Bow.001'].modifiers['Solidify'].use_flip_normals = True

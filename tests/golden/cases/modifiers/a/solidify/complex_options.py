# WP 4.2: SOLIDIFY "Complex" options: rim only (no shell), no rim, material offsets (shell and
# rim, clamped), creases from the original edges, bevel convex, the thickness clamp with and
# without the angle clamp, and the merge threshold collapsing a short edge.
# golden: startup=empty compare=evaluated:Grid,evaluated:Grid.001,evaluated:Cone,evaluated:W tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 3])
bpy.ops.transform.translate(value=(0, 0, 0.5))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.materials.new('A')
bpy.data.materials.new('B')
bpy.data.meshes['Grid'].materials.append(bpy.data.materials['A'])
bpy.data.meshes['Grid'].materials.append(bpy.data.materials['B'])
bpy.data.meshes['Grid'].attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.meshes['Grid'].attributes['crease_edge'].data[1].value = 0.8
bpy.data.meshes['Grid'].attributes['crease_edge'].data[4].value = 0.3
bpy.data.meshes['Grid'].attributes.new(name='crease_vert', type='FLOAT', domain='POINT')
bpy.data.meshes['Grid'].attributes['crease_vert'].data[5].value = 0.5
bpy.data.meshes['Grid'].attributes.new(name='bevel_weight_vert', type='FLOAT', domain='POINT')
bpy.data.meshes['Grid'].attributes['bevel_weight_vert'].data[0].value = 0.7
bpy.ops.object.duplicate()
bpy.data.objects['Grid'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Grid'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Grid'].modifiers['Solidify'].thickness = 0.2
bpy.data.objects['Grid'].modifiers['Solidify'].material_offset = 1
bpy.data.objects['Grid'].modifiers['Solidify'].material_offset_rim = 3
bpy.data.objects['Grid'].modifiers['Solidify'].bevel_convex = 0.6
bpy.data.objects['Grid.001'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Grid.001'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Grid.001'].modifiers['Solidify'].thickness = 0.2
bpy.data.objects['Grid.001'].modifiers['Solidify'].use_rim_only = True
bpy.data.objects['Grid.001'].modifiers['Solidify'].thickness_clamp = 0.5
bpy.ops.mesh.primitive_cone_add(vertices=6, location=(3, 0, 0))
bpy.data.objects['Cone'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Cone'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['Cone'].modifiers['Solidify'].thickness = 0.8
bpy.data.objects['Cone'].modifiers['Solidify'].thickness_clamp = 0.6
bpy.data.objects['Cone'].modifiers['Solidify'].use_thickness_angle_clamp = True
bpy.data.objects['Cone'].modifiers['Solidify'].use_rim = False
bpy.data.meshes.new('W')
bpy.data.meshes['W'].from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (0.5, 1.00005, 0), (2, 0, 0), (2, 1, 0)], [], [(0, 1, 2, 4, 3), (1, 5, 6, 2)])
bpy.data.objects.new('W', bpy.data.meshes['W'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W'])
bpy.data.objects['W'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['W'].modifiers['Solidify'].solidify_mode = 'NON_MANIFOLD'
bpy.data.objects['W'].modifiers['Solidify'].thickness = 0.1
bpy.data.objects['W'].modifiers['Solidify'].nonmanifold_merge_threshold = 0.6

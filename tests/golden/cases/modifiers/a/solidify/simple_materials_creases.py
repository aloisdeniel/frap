# WP 4.2: SOLIDIFY "Simple" data options: material offsets for the shell and the rim (clamped to
# the slot count), rim/inner/outer creases, and bevel convex on a grid with a bent row and on a
# cube (no rim: every edge is a manifold pair).
# golden: startup=empty compare=evaluated:Grid,evaluated:Cube tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 3])
bpy.ops.transform.translate(value=(0, 0, 0.4))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.materials.new('A')
bpy.data.materials.new('B')
bpy.data.meshes['Grid'].materials.append(bpy.data.materials['A'])
bpy.data.meshes['Grid'].materials.append(bpy.data.materials['B'])
bpy.data.objects['Grid'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Grid'].modifiers['Solidify'].thickness = 0.2
bpy.data.objects['Grid'].modifiers['Solidify'].material_offset = 1
bpy.data.objects['Grid'].modifiers['Solidify'].material_offset_rim = 5
bpy.data.objects['Grid'].modifiers['Solidify'].edge_crease_inner = 0.3
bpy.data.objects['Grid'].modifiers['Solidify'].edge_crease_outer = 0.6
bpy.data.objects['Grid'].modifiers['Solidify'].edge_crease_rim = 0.9
bpy.data.objects['Grid'].modifiers['Solidify'].bevel_convex = 0.7
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.objects['Cube'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Cube'].modifiers['Solidify'].thickness = 0.3
bpy.data.objects['Cube'].modifiers['Solidify'].bevel_convex = -0.5
bpy.data.objects['Cube'].modifiers['Solidify'].use_even_offset = True

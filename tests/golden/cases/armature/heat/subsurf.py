# WP 8.5: ARMATURE_ENVELOPE and ARMATURE_AUTO on meshes with a SUBSURF modifier (one of them
# disabled in the viewport: any SUBSURF counts, BKE_modifiers_findby_type) weigh the vertices
# at their limit-surface positions (bke::subdiv::calculate_limit_positions: Catmull-Clark
# isolated to level 1). A box (valence-3 corners) and an open grid (boundary rule) with a
# two-bone chain whose envelopes reach only part of the limit surface. Weights are copied into
# X offsets (DISPLACE along X, mid_level 0: offset = weight).
# golden: startup=empty tier=T1 tol=1e-4 compare=objects,evaluated:W.Env.Bone,evaluated:W.Env.Tip,evaluated:W.Grid.Bone,evaluated:W.Grid.Tip,evaluated:W.Auto.Bone,evaluated:W.Auto.Tip
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0, 0.1)
bpy.data.armatures['Armature'].edit_bones['Bone'].head_radius = 0.3
bpy.data.armatures['Armature'].edit_bones['Bone'].tail_radius = 0.35
bpy.data.armatures['Armature'].edit_bones['Bone'].envelope_distance = 0.6
bpy.data.armatures['Armature'].edit_bones.new('Tip')
bpy.data.armatures['Armature'].edit_bones['Tip'].head = (0, 0, 0.1)
bpy.data.armatures['Armature'].edit_bones['Tip'].tail = (0.1, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones['Tip'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.data.armatures['Armature'].edit_bones['Tip'].tail_radius = 0.2
bpy.data.armatures['Armature'].edit_bones['Tip'].envelope_distance = 0.4
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(size=1.6, location=(0, 0, -0.2), rotation=(0, 0, 0.4))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.object.name = 'Env'
bpy.data.objects['Env'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=4, size=2.0, location=(0.2, 0, -0.3), rotation=(1.3, 0, 0.2))
bpy.data.objects['Grid'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Grid'].modifiers['Subdivision'].show_viewport = False
bpy.ops.mesh.primitive_cube_add(size=1.6, location=(0, 0, -0.2), rotation=(0, 0, 0.4))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.object.name = 'Auto'
bpy.data.objects['Auto'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Env'].select_set(True)
bpy.data.objects['Grid'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE_ENVELOPE')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Auto'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.ops.object.parent_set(type='ARMATURE_AUTO')
bpy.data.objects.new('W.Env.Bone', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Env.Bone'])
bpy.data.objects['W.Env.Bone'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Env.Bone'].modifiers['D'].vertex_group = 'Bone'
bpy.data.objects['W.Env.Bone'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Env.Bone'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Env.Tip', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Env.Tip'])
bpy.data.objects['W.Env.Tip'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Env.Tip'].modifiers['D'].vertex_group = 'Tip'
bpy.data.objects['W.Env.Tip'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Env.Tip'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Grid.Bone', bpy.data.meshes['Grid'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Grid.Bone'])
bpy.data.objects['W.Grid.Bone'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Grid.Bone'].modifiers['D'].vertex_group = 'Bone'
bpy.data.objects['W.Grid.Bone'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Grid.Bone'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Grid.Tip', bpy.data.meshes['Grid'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Grid.Tip'])
bpy.data.objects['W.Grid.Tip'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Grid.Tip'].modifiers['D'].vertex_group = 'Tip'
bpy.data.objects['W.Grid.Tip'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Grid.Tip'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Auto.Bone', bpy.data.meshes['Cube.001'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Auto.Bone'])
bpy.data.objects['W.Auto.Bone'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Auto.Bone'].modifiers['D'].vertex_group = 'Bone'
bpy.data.objects['W.Auto.Bone'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Auto.Bone'].modifiers['D'].direction = 'X'
bpy.data.objects.new('W.Auto.Tip', bpy.data.meshes['Cube.001'])
bpy.context.scene.collection.objects.link(bpy.data.objects['W.Auto.Tip'])
bpy.data.objects['W.Auto.Tip'].modifiers.new(name='D', type='DISPLACE')
bpy.data.objects['W.Auto.Tip'].modifiers['D'].vertex_group = 'Tip'
bpy.data.objects['W.Auto.Tip'].modifiers['D'].mid_level = 0.0
bpy.data.objects['W.Auto.Tip'].modifiers['D'].direction = 'X'

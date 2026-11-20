# WP 6.8: import of morph targets (a hand-written glTF, `import_morph.inputs/morph.gltf`, made
# by a script): shape keys named from extras.targetNames (a string, an empty one falling back to
# the POSITION accessor's name, a number), a target without POSITION skipped, a sparse target,
# a second primitive without targets for the first keys; the mesh's weights (beyond [0, 1]:
# slider ranges widened) or the node's as key values; nodes with equal weights share their
# mesh, a node whose weights are animated gets its own (its animation holds the node's weights,
# so it evaluates to them). Key values, slider ranges and points
# are copied into empties to be compared.
# golden: startup=empty order=canonical tier=T1 tol=1e-5 compare=objects,context,mesh:Plane,mesh:Plane.001,mesh:Plane.002
import bpy
bpy.ops.import_scene.gltf(filepath='morph.gltf')
bpy.data.objects['A'].data.name
bpy.data.objects['B'].data.name
bpy.data.objects['C'].data.name
bpy.data.objects['D'].data.name
bpy.data.meshes['Plane'].shape_keys.name
bpy.data.meshes['Plane'].shape_keys.key_blocks['OnlyNormal'].name
bpy.data.objects.new('K00', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K00'])
bpy.data.objects['K00'].location = (bpy.data.meshes['Plane'].shape_keys.key_blocks['Basis'].value, bpy.data.meshes['Plane'].shape_keys.key_blocks['Basis'].slider_min, bpy.data.meshes['Plane'].shape_keys.key_blocks['Basis'].slider_max)
bpy.data.objects['K00'].rotation_euler = bpy.data.meshes['Plane'].shape_keys.key_blocks['Basis'].data[2].co
bpy.data.objects['K00'].scale = bpy.data.meshes['Plane'].shape_keys.key_blocks['Basis'].data[4].co
bpy.data.meshes['Plane'].shape_keys.key_blocks['Basis'].relative_key.name
bpy.data.objects.new('K01', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K01'])
bpy.data.objects['K01'].location = (bpy.data.meshes['Plane'].shape_keys.key_blocks['Smile'].value, bpy.data.meshes['Plane'].shape_keys.key_blocks['Smile'].slider_min, bpy.data.meshes['Plane'].shape_keys.key_blocks['Smile'].slider_max)
bpy.data.objects['K01'].rotation_euler = bpy.data.meshes['Plane'].shape_keys.key_blocks['Smile'].data[2].co
bpy.data.objects['K01'].scale = bpy.data.meshes['Plane'].shape_keys.key_blocks['Smile'].data[4].co
bpy.data.meshes['Plane'].shape_keys.key_blocks['Smile'].relative_key.name
bpy.data.objects.new('K02', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K02'])
bpy.data.objects['K02'].location = (bpy.data.meshes['Plane'].shape_keys.key_blocks['AccName'].value, bpy.data.meshes['Plane'].shape_keys.key_blocks['AccName'].slider_min, bpy.data.meshes['Plane'].shape_keys.key_blocks['AccName'].slider_max)
bpy.data.objects['K02'].rotation_euler = bpy.data.meshes['Plane'].shape_keys.key_blocks['AccName'].data[2].co
bpy.data.objects['K02'].scale = bpy.data.meshes['Plane'].shape_keys.key_blocks['AccName'].data[4].co
bpy.data.meshes['Plane'].shape_keys.key_blocks['AccName'].relative_key.name
bpy.data.objects.new('K03', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K03'])
bpy.data.objects['K03'].location = (bpy.data.meshes['Plane'].shape_keys.key_blocks['7'].value, bpy.data.meshes['Plane'].shape_keys.key_blocks['7'].slider_min, bpy.data.meshes['Plane'].shape_keys.key_blocks['7'].slider_max)
bpy.data.objects['K03'].rotation_euler = bpy.data.meshes['Plane'].shape_keys.key_blocks['7'].data[2].co
bpy.data.objects['K03'].scale = bpy.data.meshes['Plane'].shape_keys.key_blocks['7'].data[4].co
bpy.data.meshes['Plane'].shape_keys.key_blocks['7'].relative_key.name
bpy.data.objects.new('K04', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K04'])
bpy.data.objects['K04'].location = (bpy.data.meshes['Plane'].shape_keys.key_blocks['Dup'].value, bpy.data.meshes['Plane'].shape_keys.key_blocks['Dup'].slider_min, bpy.data.meshes['Plane'].shape_keys.key_blocks['Dup'].slider_max)
bpy.data.objects['K04'].rotation_euler = bpy.data.meshes['Plane'].shape_keys.key_blocks['Dup'].data[2].co
bpy.data.objects['K04'].scale = bpy.data.meshes['Plane'].shape_keys.key_blocks['Dup'].data[4].co
bpy.data.meshes['Plane'].shape_keys.key_blocks['Dup'].relative_key.name
bpy.data.objects.new('K10', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K10'])
bpy.data.objects['K10'].location = (bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Basis'].value, bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Basis'].slider_min, bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Basis'].slider_max)
bpy.data.objects['K10'].rotation_euler = bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Basis'].data[2].co
bpy.data.objects['K10'].scale = bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Basis'].data[4].co
bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Basis'].relative_key.name
bpy.data.objects.new('K11', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K11'])
bpy.data.objects['K11'].location = (bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Smile'].value, bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Smile'].slider_min, bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Smile'].slider_max)
bpy.data.objects['K11'].rotation_euler = bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Smile'].data[2].co
bpy.data.objects['K11'].scale = bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Smile'].data[4].co
bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Smile'].relative_key.name
bpy.data.objects.new('K12', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K12'])
bpy.data.objects['K12'].location = (bpy.data.meshes['Plane.001'].shape_keys.key_blocks['AccName'].value, bpy.data.meshes['Plane.001'].shape_keys.key_blocks['AccName'].slider_min, bpy.data.meshes['Plane.001'].shape_keys.key_blocks['AccName'].slider_max)
bpy.data.objects['K12'].rotation_euler = bpy.data.meshes['Plane.001'].shape_keys.key_blocks['AccName'].data[2].co
bpy.data.objects['K12'].scale = bpy.data.meshes['Plane.001'].shape_keys.key_blocks['AccName'].data[4].co
bpy.data.meshes['Plane.001'].shape_keys.key_blocks['AccName'].relative_key.name
bpy.data.objects.new('K13', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K13'])
bpy.data.objects['K13'].location = (bpy.data.meshes['Plane.001'].shape_keys.key_blocks['7'].value, bpy.data.meshes['Plane.001'].shape_keys.key_blocks['7'].slider_min, bpy.data.meshes['Plane.001'].shape_keys.key_blocks['7'].slider_max)
bpy.data.objects['K13'].rotation_euler = bpy.data.meshes['Plane.001'].shape_keys.key_blocks['7'].data[2].co
bpy.data.objects['K13'].scale = bpy.data.meshes['Plane.001'].shape_keys.key_blocks['7'].data[4].co
bpy.data.meshes['Plane.001'].shape_keys.key_blocks['7'].relative_key.name
bpy.data.objects.new('K14', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K14'])
bpy.data.objects['K14'].location = (bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Dup'].value, bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Dup'].slider_min, bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Dup'].slider_max)
bpy.data.objects['K14'].rotation_euler = bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Dup'].data[2].co
bpy.data.objects['K14'].scale = bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Dup'].data[4].co
bpy.data.meshes['Plane.001'].shape_keys.key_blocks['Dup'].relative_key.name
bpy.data.objects.new('K20', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K20'])
bpy.data.objects['K20'].location = (bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Basis'].value, bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Basis'].slider_min, bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Basis'].slider_max)
bpy.data.objects['K20'].rotation_euler = bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Basis'].data[2].co
bpy.data.objects['K20'].scale = bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Basis'].data[4].co
bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Basis'].relative_key.name
bpy.data.objects.new('K21', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K21'])
bpy.data.objects['K21'].location = (bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Smile'].value, bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Smile'].slider_min, bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Smile'].slider_max)
bpy.data.objects['K21'].rotation_euler = bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Smile'].data[2].co
bpy.data.objects['K21'].scale = bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Smile'].data[4].co
bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Smile'].relative_key.name
bpy.data.objects.new('K22', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K22'])
bpy.data.objects['K22'].location = (bpy.data.meshes['Plane.002'].shape_keys.key_blocks['AccName'].value, bpy.data.meshes['Plane.002'].shape_keys.key_blocks['AccName'].slider_min, bpy.data.meshes['Plane.002'].shape_keys.key_blocks['AccName'].slider_max)
bpy.data.objects['K22'].rotation_euler = bpy.data.meshes['Plane.002'].shape_keys.key_blocks['AccName'].data[2].co
bpy.data.objects['K22'].scale = bpy.data.meshes['Plane.002'].shape_keys.key_blocks['AccName'].data[4].co
bpy.data.meshes['Plane.002'].shape_keys.key_blocks['AccName'].relative_key.name
bpy.data.objects.new('K23', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K23'])
bpy.data.objects['K23'].location = (bpy.data.meshes['Plane.002'].shape_keys.key_blocks['7'].value, bpy.data.meshes['Plane.002'].shape_keys.key_blocks['7'].slider_min, bpy.data.meshes['Plane.002'].shape_keys.key_blocks['7'].slider_max)
bpy.data.objects['K23'].rotation_euler = bpy.data.meshes['Plane.002'].shape_keys.key_blocks['7'].data[2].co
bpy.data.objects['K23'].scale = bpy.data.meshes['Plane.002'].shape_keys.key_blocks['7'].data[4].co
bpy.data.meshes['Plane.002'].shape_keys.key_blocks['7'].relative_key.name
bpy.data.objects.new('K24', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['K24'])
bpy.data.objects['K24'].location = (bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Dup'].value, bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Dup'].slider_min, bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Dup'].slider_max)
bpy.data.objects['K24'].rotation_euler = bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Dup'].data[2].co
bpy.data.objects['K24'].scale = bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Dup'].data[4].co
bpy.data.meshes['Plane.002'].shape_keys.key_blocks['Dup'].relative_key.name

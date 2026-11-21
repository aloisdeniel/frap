# M5+M6 stabilization round 7: an animation write clamps to the property's dynamic range
# (`BKE_animsys_write_to_rna_path` -> `RNA_property_float_clamp` -> the range callback), not
# its static one. ShapeKey.value: the block's slider_min..slider_max (`rna_ShapeKey_value_range`;
# keys -0.5@1, 1.0@9, 2.5@12 with sliders -1..3 read -0.5, -0.4209 (frame 4), 2.5; a key past
# the sliders is clamped to them), so the evaluated mesh follows the unclamped weight. A
# standard socket's default_value (`rna_NodeSocketStandard_float_range`): unbounded (Metallic
# keyed to 2.5 reads 2.5). Values are copied into empties to be compared.
# golden: startup=factory tier=T1 tol=1e-4 compare=objects,evaluated:Plane
import bpy
bpy.ops.mesh.primitive_plane_add()
bpy.data.objects['Plane'].shape_key_add(name='Basis')
bpy.data.objects['Plane'].shape_key_add(name='K')
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].data[0].co = (-1.0, -1.0, 1.0)
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].slider_min = -1.0
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].slider_max = 3.0
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value = -0.5
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].keyframe_insert('value', frame=1)
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value = 1.0
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].keyframe_insert('value', frame=9)
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value = 2.5
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].keyframe_insert('value', frame=12)
bpy.data.meshes['Plane'].shape_keys.animation_data.action.layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(14, 7.0)
bpy.context.scene.frame_set(1)
bpy.data.objects.new('at1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['at1'])
bpy.data.objects['at1'].location = (bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value, 0.0, 0.0)
bpy.context.scene.frame_set(4)
bpy.data.objects.new('at4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['at4'])
bpy.data.objects['at4'].location = (bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value, 0.0, 0.0)
bpy.context.scene.frame_set(12)
bpy.data.objects.new('at12', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['at12'])
bpy.data.objects['at12'].location = (bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value, 0.0, 0.0)
bpy.context.scene.frame_set(14)
bpy.data.objects.new('at14', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['at14'])
bpy.data.objects['at14'].location = (bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value, 0.0, 0.0)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 0.0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].keyframe_insert('default_value', frame=1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 2.5
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].keyframe_insert('default_value', frame=3)
bpy.context.scene.frame_set(3)
bpy.data.objects.new('metal', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['metal'])
bpy.data.objects['metal'].location = (bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value, 0.0, 0.0)
bpy.context.scene.frame_set(4)

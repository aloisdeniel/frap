# M5+M6 stabilization round 7: animated shape key weights outside 0..1 (sliders -1..3): the
# animation writes clamp to the block's slider range, not the static 0..1 (`RNA_property_float_
# clamp` with `rna_ShapeKey_value_range`). Exports: the default ACTIONS mode (sampled weights)
# and SCENE mode (the scene range baked), with the mesh's weights (-1, as Blender writes them).
# The SCENE export is imported back: playing it at frame 1 and 6 gives the exported weights
# (the import widens the sliders), copied into empties to be compared.
# golden: startup=factory tier=T2 tol=1e-4 compare=objects,glb:out/actions.glb,glb:out/scene.glb glb_structure
import bpy
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Cube'].hide_render = True
bpy.context.scene.frame_end = 6
bpy.ops.mesh.primitive_plane_add(location=(3.0, 0.0, 0.0))
bpy.data.objects['Plane'].shape_key_add(name='Basis')
bpy.data.objects['Plane'].shape_key_add(name='K')
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].data[0].co = (-1.0, -1.0, 1.0)
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].slider_min = -1.0
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].slider_max = 3.0
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value = -1.0
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].keyframe_insert('value', frame=1)
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value = 2.5
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].keyframe_insert('value', frame=6)
bpy.data.meshes['Plane'].shape_keys.animation_data.action.layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].interpolation = 'LINEAR'
bpy.context.scene.frame_set(2)
bpy.ops.export_scene.gltf(filepath='out/actions.glb', use_selection=True)
bpy.ops.export_scene.gltf(filepath='out/scene.glb', use_selection=True, export_animation_mode='SCENE')
bpy.data.objects['Plane'].name = 'Source'
bpy.data.objects['Source'].hide_render = True
bpy.ops.import_scene.gltf(filepath='out/scene.glb')
bpy.context.scene.frame_set(1)
bpy.data.objects.new('back1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['back1'])
bpy.data.objects['back1'].location = (bpy.data.objects['Plane'].data.shape_keys.key_blocks['K'].value, bpy.data.objects['Plane'].data.shape_keys.key_blocks['K'].slider_min, bpy.data.objects['Plane'].data.shape_keys.key_blocks['K'].slider_max)
bpy.context.scene.frame_set(6)
bpy.data.objects.new('back6', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['back6'])
bpy.data.objects['back6'].location = (bpy.data.objects['Plane'].data.shape_keys.key_blocks['K'].value, bpy.data.objects['Plane'].data.shape_keys.key_blocks['K'].slider_min, bpy.data.objects['Plane'].data.shape_keys.key_blocks['K'].slider_max)

# WP 6.7: sampling options. A slotted action shared by two objects (one slot each) becomes one
# animation; export_bake_animation also samples the objects without an action (a child of an
# animated parent moves); export_optimize_animation_size=False keeps every sample and every
# keyed channel; export_optimize_animation_keep_anim_object keeps constant baked channels as two
# STEP keys; export_sampling_interpolation_fallback='STEP' writes STEP samplers; an action with
# a manual frame range (use_frame_range) is sampled over that range.
# golden: startup=factory tier=T2 tol=1e-5 compare=glb:out/shared.glb,glb:out/bake.glb,glb:out/noopt.glb,glb:out/keep.glb,glb:out/step.glb,glb:out/range_curves.glb
import bpy
bpy.ops.object.empty_add(location=(0.0, 3.0, 0.0))
bpy.ops.mesh.primitive_plane_add(location=(0.0, 0.0, -1.0))
bpy.data.objects['Plane'].parent = bpy.data.objects['Cube']
bpy.data.actions.new('Shared')
bpy.data.actions['Shared'].slots.new('OBJECT', 'Cube')
bpy.data.actions['Shared'].slots.new('OBJECT', 'Empty')
bpy.data.objects['Cube'].animation_data_create()
bpy.data.objects['Cube'].animation_data.action = bpy.data.actions['Shared']
bpy.data.objects['Cube'].animation_data.action_slot = bpy.data.actions['Shared'].slots['OBCube']
bpy.data.objects['Empty'].animation_data_create()
bpy.data.objects['Empty'].animation_data.action = bpy.data.actions['Shared']
bpy.data.objects['Empty'].animation_data.action_slot = bpy.data.actions['Shared'].slots['OBEmpty']
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location = (0.0, 0.0, 2.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=5)
bpy.data.objects['Empty'].keyframe_insert('scale', frame=2)
bpy.data.objects['Empty'].scale = (2.0, 2.0, 2.0)
bpy.data.objects['Empty'].keyframe_insert('scale', frame=8)
bpy.data.objects['Empty'].keyframe_insert('location', frame=2)
bpy.ops.export_scene.gltf(filepath='out/shared.glb')
bpy.ops.export_scene.gltf(filepath='out/bake.glb', export_bake_animation=True)
bpy.ops.export_scene.gltf(filepath='out/noopt.glb', export_optimize_animation_size=False)
bpy.ops.export_scene.gltf(filepath='out/keep.glb', export_bake_animation=True, export_optimize_animation_keep_anim_object=True)
bpy.ops.export_scene.gltf(filepath='out/step.glb', export_sampling_interpolation_fallback='STEP')
bpy.data.actions['Shared'].use_frame_range = True
bpy.data.actions['Shared'].frame_start = 2
bpy.data.actions['Shared'].frame_end = 4
bpy.ops.export_scene.gltf(filepath='out/range_curves.glb', export_force_sampling=False)

# WP 7.6: ShapeKey.value animated together with slider_max. Blender writes a layered action's
# channels in the order of its EvaluationResult map (blender::Map slot order, not F-curve
# order), and the value is clamped to the sliders in force when it is written: with keys
# value 0.5@1 / 2.5@10 and slider_max 1@1 / 3@10, at frame 10 the blocks whose slider_max comes
# first keep 2.5 (K, A, G, Key) and the others are cut to the previous 1.0 (B, F, Smile); with a
# third channel (slider_min) the map has 8 slots: A3 keeps 2.5, K3 is cut. The values go into probe
# empties named after the blocks.
# golden: startup=factory tier=T1 tol=1e-5 compare=objects
import bpy
bpy.ops.mesh.primitive_plane_add()
bpy.context.object.name = 'K'
bpy.context.object.data.name = 'K'
bpy.data.objects['K'].shape_key_add(name='Basis')
bpy.data.objects['K'].shape_key_add(name='K')
bpy.data.meshes['K'].shape_keys.key_blocks['K'].value = 0.5
bpy.data.meshes['K'].shape_keys.key_blocks['K'].keyframe_insert('value', frame=1)
bpy.data.meshes['K'].shape_keys.key_blocks['K'].slider_max = 3.0
bpy.data.meshes['K'].shape_keys.key_blocks['K'].value = 2.5
bpy.data.meshes['K'].shape_keys.key_blocks['K'].keyframe_insert('value', frame=10)
bpy.data.meshes['K'].shape_keys.key_blocks['K'].slider_max = 1.0
bpy.data.meshes['K'].shape_keys.key_blocks['K'].keyframe_insert('slider_max', frame=1)
bpy.data.meshes['K'].shape_keys.key_blocks['K'].slider_max = 3.0
bpy.data.meshes['K'].shape_keys.key_blocks['K'].keyframe_insert('slider_max', frame=10)
bpy.data.meshes['K'].shape_keys.key_blocks['K'].slider_max = 1.0
bpy.ops.mesh.primitive_plane_add()
bpy.context.object.name = 'A'
bpy.context.object.data.name = 'A'
bpy.data.objects['A'].shape_key_add(name='Basis')
bpy.data.objects['A'].shape_key_add(name='A')
bpy.data.meshes['A'].shape_keys.key_blocks['A'].value = 0.5
bpy.data.meshes['A'].shape_keys.key_blocks['A'].keyframe_insert('value', frame=1)
bpy.data.meshes['A'].shape_keys.key_blocks['A'].slider_max = 3.0
bpy.data.meshes['A'].shape_keys.key_blocks['A'].value = 2.5
bpy.data.meshes['A'].shape_keys.key_blocks['A'].keyframe_insert('value', frame=10)
bpy.data.meshes['A'].shape_keys.key_blocks['A'].slider_max = 1.0
bpy.data.meshes['A'].shape_keys.key_blocks['A'].keyframe_insert('slider_max', frame=1)
bpy.data.meshes['A'].shape_keys.key_blocks['A'].slider_max = 3.0
bpy.data.meshes['A'].shape_keys.key_blocks['A'].keyframe_insert('slider_max', frame=10)
bpy.data.meshes['A'].shape_keys.key_blocks['A'].slider_max = 1.0
bpy.ops.mesh.primitive_plane_add()
bpy.context.object.name = 'B'
bpy.context.object.data.name = 'B'
bpy.data.objects['B'].shape_key_add(name='Basis')
bpy.data.objects['B'].shape_key_add(name='B')
bpy.data.meshes['B'].shape_keys.key_blocks['B'].value = 0.5
bpy.data.meshes['B'].shape_keys.key_blocks['B'].keyframe_insert('value', frame=1)
bpy.data.meshes['B'].shape_keys.key_blocks['B'].slider_max = 3.0
bpy.data.meshes['B'].shape_keys.key_blocks['B'].value = 2.5
bpy.data.meshes['B'].shape_keys.key_blocks['B'].keyframe_insert('value', frame=10)
bpy.data.meshes['B'].shape_keys.key_blocks['B'].slider_max = 1.0
bpy.data.meshes['B'].shape_keys.key_blocks['B'].keyframe_insert('slider_max', frame=1)
bpy.data.meshes['B'].shape_keys.key_blocks['B'].slider_max = 3.0
bpy.data.meshes['B'].shape_keys.key_blocks['B'].keyframe_insert('slider_max', frame=10)
bpy.data.meshes['B'].shape_keys.key_blocks['B'].slider_max = 1.0
bpy.ops.mesh.primitive_plane_add()
bpy.context.object.name = 'G'
bpy.context.object.data.name = 'G'
bpy.data.objects['G'].shape_key_add(name='Basis')
bpy.data.objects['G'].shape_key_add(name='G')
bpy.data.meshes['G'].shape_keys.key_blocks['G'].value = 0.5
bpy.data.meshes['G'].shape_keys.key_blocks['G'].keyframe_insert('value', frame=1)
bpy.data.meshes['G'].shape_keys.key_blocks['G'].slider_max = 3.0
bpy.data.meshes['G'].shape_keys.key_blocks['G'].value = 2.5
bpy.data.meshes['G'].shape_keys.key_blocks['G'].keyframe_insert('value', frame=10)
bpy.data.meshes['G'].shape_keys.key_blocks['G'].slider_max = 1.0
bpy.data.meshes['G'].shape_keys.key_blocks['G'].keyframe_insert('slider_max', frame=1)
bpy.data.meshes['G'].shape_keys.key_blocks['G'].slider_max = 3.0
bpy.data.meshes['G'].shape_keys.key_blocks['G'].keyframe_insert('slider_max', frame=10)
bpy.data.meshes['G'].shape_keys.key_blocks['G'].slider_max = 1.0
bpy.ops.mesh.primitive_plane_add()
bpy.context.object.name = 'Key'
bpy.context.object.data.name = 'Key'
bpy.data.objects['Key'].shape_key_add(name='Basis')
bpy.data.objects['Key'].shape_key_add(name='Key')
bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].value = 0.5
bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].keyframe_insert('value', frame=1)
bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].slider_max = 3.0
bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].value = 2.5
bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].keyframe_insert('value', frame=10)
bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].slider_max = 1.0
bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].keyframe_insert('slider_max', frame=1)
bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].slider_max = 3.0
bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].keyframe_insert('slider_max', frame=10)
bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].slider_max = 1.0
bpy.ops.mesh.primitive_plane_add()
bpy.context.object.name = 'Smile'
bpy.context.object.data.name = 'Smile'
bpy.data.objects['Smile'].shape_key_add(name='Basis')
bpy.data.objects['Smile'].shape_key_add(name='Smile')
bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].value = 0.5
bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].keyframe_insert('value', frame=1)
bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].slider_max = 3.0
bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].value = 2.5
bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].keyframe_insert('value', frame=10)
bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].slider_max = 1.0
bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].keyframe_insert('slider_max', frame=1)
bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].slider_max = 3.0
bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].keyframe_insert('slider_max', frame=10)
bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].slider_max = 1.0
bpy.ops.mesh.primitive_plane_add()
bpy.context.object.name = 'F'
bpy.context.object.data.name = 'F'
bpy.data.objects['F'].shape_key_add(name='Basis')
bpy.data.objects['F'].shape_key_add(name='F')
bpy.data.meshes['F'].shape_keys.key_blocks['F'].value = 0.5
bpy.data.meshes['F'].shape_keys.key_blocks['F'].keyframe_insert('value', frame=1)
bpy.data.meshes['F'].shape_keys.key_blocks['F'].slider_max = 3.0
bpy.data.meshes['F'].shape_keys.key_blocks['F'].value = 2.5
bpy.data.meshes['F'].shape_keys.key_blocks['F'].keyframe_insert('value', frame=10)
bpy.data.meshes['F'].shape_keys.key_blocks['F'].slider_max = 1.0
bpy.data.meshes['F'].shape_keys.key_blocks['F'].keyframe_insert('slider_max', frame=1)
bpy.data.meshes['F'].shape_keys.key_blocks['F'].slider_max = 3.0
bpy.data.meshes['F'].shape_keys.key_blocks['F'].keyframe_insert('slider_max', frame=10)
bpy.data.meshes['F'].shape_keys.key_blocks['F'].slider_max = 1.0
bpy.ops.mesh.primitive_plane_add()
bpy.context.object.name = 'A3'
bpy.context.object.data.name = 'A3'
bpy.data.objects['A3'].shape_key_add(name='Basis')
bpy.data.objects['A3'].shape_key_add(name='A3')
bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].value = 0.5
bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].keyframe_insert('value', frame=1)
bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].slider_max = 3.0
bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].value = 2.5
bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].keyframe_insert('value', frame=10)
bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].slider_max = 1.0
bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].keyframe_insert('slider_max', frame=1)
bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].slider_max = 3.0
bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].keyframe_insert('slider_max', frame=10)
bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].slider_max = 1.0
bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].keyframe_insert('slider_min', frame=1)
bpy.ops.mesh.primitive_plane_add()
bpy.context.object.name = 'K3'
bpy.context.object.data.name = 'K3'
bpy.data.objects['K3'].shape_key_add(name='Basis')
bpy.data.objects['K3'].shape_key_add(name='K3')
bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].value = 0.5
bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].keyframe_insert('value', frame=1)
bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].slider_max = 3.0
bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].value = 2.5
bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].keyframe_insert('value', frame=10)
bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].slider_max = 1.0
bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].keyframe_insert('slider_max', frame=1)
bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].slider_max = 3.0
bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].keyframe_insert('slider_max', frame=10)
bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].slider_max = 1.0
bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].keyframe_insert('slider_min', frame=1)
bpy.context.scene.frame_set(1)
bpy.context.scene.frame_set(10)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('v_K', None))
bpy.data.objects['v_K'].location = (bpy.data.meshes['K'].shape_keys.key_blocks['K'].value, bpy.data.meshes['K'].shape_keys.key_blocks['K'].slider_max, bpy.data.meshes['K'].shape_keys.key_blocks['K'].slider_min)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('v_A', None))
bpy.data.objects['v_A'].location = (bpy.data.meshes['A'].shape_keys.key_blocks['A'].value, bpy.data.meshes['A'].shape_keys.key_blocks['A'].slider_max, bpy.data.meshes['A'].shape_keys.key_blocks['A'].slider_min)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('v_B', None))
bpy.data.objects['v_B'].location = (bpy.data.meshes['B'].shape_keys.key_blocks['B'].value, bpy.data.meshes['B'].shape_keys.key_blocks['B'].slider_max, bpy.data.meshes['B'].shape_keys.key_blocks['B'].slider_min)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('v_G', None))
bpy.data.objects['v_G'].location = (bpy.data.meshes['G'].shape_keys.key_blocks['G'].value, bpy.data.meshes['G'].shape_keys.key_blocks['G'].slider_max, bpy.data.meshes['G'].shape_keys.key_blocks['G'].slider_min)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('v_Key', None))
bpy.data.objects['v_Key'].location = (bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].value, bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].slider_max, bpy.data.meshes['Key'].shape_keys.key_blocks['Key'].slider_min)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('v_Smile', None))
bpy.data.objects['v_Smile'].location = (bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].value, bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].slider_max, bpy.data.meshes['Smile'].shape_keys.key_blocks['Smile'].slider_min)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('v_F', None))
bpy.data.objects['v_F'].location = (bpy.data.meshes['F'].shape_keys.key_blocks['F'].value, bpy.data.meshes['F'].shape_keys.key_blocks['F'].slider_max, bpy.data.meshes['F'].shape_keys.key_blocks['F'].slider_min)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('v_A3', None))
bpy.data.objects['v_A3'].location = (bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].value, bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].slider_max, bpy.data.meshes['A3'].shape_keys.key_blocks['A3'].slider_min)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('v_K3', None))
bpy.data.objects['v_K3'].location = (bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].value, bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].slider_max, bpy.data.meshes['K3'].shape_keys.key_blocks['K3'].slider_min)
bpy.context.scene.frame_set(5)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('v5_K', None))
bpy.data.objects['v5_K'].location = (bpy.data.meshes['K'].shape_keys.key_blocks['K'].value, bpy.data.meshes['K'].shape_keys.key_blocks['K'].slider_max, bpy.data.meshes['K'].shape_keys.key_blocks['K'].slider_min)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('v5_A', None))
bpy.data.objects['v5_A'].location = (bpy.data.meshes['A'].shape_keys.key_blocks['A'].value, bpy.data.meshes['A'].shape_keys.key_blocks['A'].slider_max, bpy.data.meshes['A'].shape_keys.key_blocks['A'].slider_min)

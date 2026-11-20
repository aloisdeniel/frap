# WP 6.4: bpy_struct.keyframe_insert / keyframe_delete on an object: CubeAction, slot OBCube,
# layer "Layer", group "Object Transforms" (looked up by key, so a wrong name fails the line),
# BEZIER + AUTO_CLAMPED keys with recalculated handles, frame=None keys at frame_current,
# single indices, discrete (boolean) and integer-free channels, keytype, an explicit group,
# INSERTKEY_NEEDED / REPLACE / AVAILABLE, deleting keys (empty F-curves removed), and Blender's
# argument errors. Names and counts are moved into object names and transforms to be compared.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:CubeAction,objects
import bpy
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location = (2.0, -1.0, 0.5)
bpy.data.objects['Cube'].keyframe_insert('location', frame=11)
bpy.data.objects['Cube'].location.z = 3.0
bpy.data.objects['Cube'].keyframe_insert('location', index=2, frame=6)
bpy.data.objects['Cube'].rotation_euler = (0.5, 0.0, 1.0)
bpy.context.scene.frame_current = 4
bpy.data.objects['Cube'].keyframe_insert('rotation_euler')
bpy.data.objects['Cube'].keyframe_insert('scale', frame=20, keytype='BREAKDOWN')
bpy.data.objects['Cube'].hide_render = True
bpy.data.objects['Cube'].keyframe_insert('hide_render', frame=3)
bpy.data.objects['Cube'].hide_render = False
bpy.data.objects['Cube'].keyframe_insert('hide_render', frame=7)
bpy.data.objects['Cube'].keyframe_insert('delta_location', frame=2, group='Deltas')
bpy.data.objects['Cube'].keyframe_insert('location', frame=11, options={'INSERTKEY_NEEDED'})
bpy.data.objects['Cube'].keyframe_insert('location', frame=30, options={'INSERTKEY_REPLACE'})
bpy.data.objects['Cube'].keyframe_insert('color', frame=5, options={'INSERTKEY_AVAILABLE'})
bpy.data.objects['Cube'].location.x = 5.0
bpy.data.objects['Cube'].keyframe_insert('location', frame=11, options={'INSERTKEY_REPLACE'})
bpy.data.objects['Cube'].keyframe_insert('location', 2, 20)
bpy.data.objects['Cube'].keyframe_insert('location', index=3)
bpy.data.objects['Cube'].keyframe_insert('location[0]')
bpy.data.objects['Cube'].keyframe_insert('nope')
bpy.data.objects['Cube'].keyframe_insert('name')
bpy.data.objects['Cube'].keyframe_insert('hide_render', index=1)
bpy.data.objects['Cube'].keyframe_insert('data.name')
bpy.data.objects['Cube'].keyframe_insert('location', keytype='NOPE')
bpy.data.objects['Cube'].keyframe_insert('location', options={'NOPE'})
bpy.data.actions['CubeAction'].layers['Layer'].strips[0].channelbags[0].groups['Object Transforms']
bpy.data.actions['CubeAction'].layers['Layer'].strips[0].channelbags[0].groups['Deltas']
bpy.data.actions['CubeAction'].slots['OBCube']
bpy.data.objects.new('group_' + bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('hide_render').group.name, None)
bpy.data.objects.new('key_' + bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('scale', index=1).keyframe_points[0].type, None)
bpy.data.objects.new('ipo_' + bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('hide_render').keyframe_points[0].interpolation, None)
bpy.data.objects.new('slot_' + bpy.data.objects['Cube'].animation_data.action_slot.identifier, None)
bpy.data.objects['Cube'].keyframe_delete('location', frame=6)
bpy.data.objects['Cube'].keyframe_delete('location', index=0, frame=1)
bpy.data.objects['Cube'].keyframe_delete('delta_location', frame=2)
bpy.data.objects['Cube'].keyframe_delete('scale', frame=99)
bpy.data.objects['Camera'].keyframe_delete('location')
bpy.data.objects['Camera'].animation_data_create()
bpy.data.objects['Camera'].keyframe_delete('location')
bpy.data.objects['Camera'].location = (bpy.data.actions['CubeAction'].users, bpy.data.actions['CubeAction'].layers[0].strips[0].channelbags[0].fcurves.find('location', index=0).keyframe_points[0].co.x, bpy.context.scene.frame_current)
bpy.context.view_layer.update()

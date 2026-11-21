# M5+M6 stabilization: a session boundary animates only the datablocks of the window's
# depsgraph (`frap_eval::animate::depsgraph_roots`), not every ID of bpy.data. Each object is
# keyed (0,0,0)@1 -> (4,0,0)@11, frame_set(1) applies frame 1, a manual location (9,9,9)
# follows, then frame_set(6): an object unlinked from every collection (the Cube), an object
# linked only into another scene, one with hide_viewport and one in an excluded collection
# keep (9,9,9); a hide_set object, a parent in no collection and the child are animated. The
# shape key of an orphan mesh (its object removed) keeps its manual value, copied into an empty
# to be compared. The objects outside the depsgraph are copied into probe empties and removed
# before the dump (their matrix_world is stale in Blender, D-96).
# golden: startup=factory tier=T1 tol=1e-5 compare=objects
import bpy
bpy.data.scenes.new('Other')
bpy.data.objects.new('InOther', None)
bpy.data.scenes['Other'].collection.objects.link(bpy.data.objects['InOther'])
bpy.data.objects.new('HideViewport', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['HideViewport'])
bpy.data.objects.new('HideSet', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['HideSet'])
bpy.data.collections.new('Excluded')
bpy.context.scene.collection.children.link(bpy.data.collections['Excluded'])
bpy.data.objects.new('InExcluded', None)
bpy.data.collections['Excluded'].objects.link(bpy.data.objects['InExcluded'])
bpy.context.view_layer.layer_collection.children['Excluded'].exclude = True
bpy.data.objects.new('ParentOnly', None)
bpy.data.objects.new('Child', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Child'])
bpy.data.objects['Child'].parent = bpy.data.objects['ParentOnly']
bpy.ops.mesh.primitive_plane_add()
bpy.data.objects['Plane'].shape_key_add(name='Basis')
bpy.data.objects['Plane'].shape_key_add(name='K')
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value = 0.0
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].keyframe_insert('value', frame=1)
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value = 1.0
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].keyframe_insert('value', frame=11)
bpy.data.objects.remove(bpy.data.objects['Plane'])
bpy.data.objects['Cube'].location = (0.0, 0.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location = (4.0, 0.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=11)
bpy.data.objects['InOther'].keyframe_insert('location', frame=1)
bpy.data.objects['InOther'].location = (4.0, 0.0, 0.0)
bpy.data.objects['InOther'].keyframe_insert('location', frame=11)
bpy.data.objects['HideViewport'].keyframe_insert('location', frame=1)
bpy.data.objects['HideViewport'].location = (4.0, 0.0, 0.0)
bpy.data.objects['HideViewport'].keyframe_insert('location', frame=11)
bpy.data.objects['HideSet'].keyframe_insert('location', frame=1)
bpy.data.objects['HideSet'].location = (4.0, 0.0, 0.0)
bpy.data.objects['HideSet'].keyframe_insert('location', frame=11)
bpy.data.objects['InExcluded'].keyframe_insert('location', frame=1)
bpy.data.objects['InExcluded'].location = (4.0, 0.0, 0.0)
bpy.data.objects['InExcluded'].keyframe_insert('location', frame=11)
bpy.data.objects['ParentOnly'].keyframe_insert('location', frame=1)
bpy.data.objects['ParentOnly'].location = (4.0, 0.0, 0.0)
bpy.data.objects['ParentOnly'].keyframe_insert('location', frame=11)
bpy.data.objects['Child'].keyframe_insert('location', frame=1)
bpy.data.objects['Child'].location = (4.0, 0.0, 0.0)
bpy.data.objects['Child'].keyframe_insert('location', frame=11)
bpy.data.objects['HideViewport'].hide_viewport = True
bpy.data.objects['HideSet'].hide_set(True)
bpy.context.scene.frame_set(1)
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Cube'])
bpy.data.objects['Cube'].location = (9.0, 9.0, 9.0)
bpy.data.objects['InOther'].location = (9.0, 9.0, 9.0)
bpy.data.objects['HideViewport'].location = (9.0, 9.0, 9.0)
bpy.data.objects['HideSet'].location = (9.0, 9.0, 9.0)
bpy.data.objects['InExcluded'].location = (9.0, 9.0, 9.0)
bpy.data.objects['ParentOnly'].location = (9.0, 9.0, 9.0)
bpy.data.objects['Child'].location = (9.0, 9.0, 9.0)
bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value = 0.25
bpy.context.scene.frame_set(6)
bpy.data.objects['Camera'].scale = (bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value, 1.0, 1.0)
bpy.data.objects.new('P_Cube', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Cube'])
bpy.data.objects['P_Cube'].location = bpy.data.objects['Cube'].location
bpy.data.objects.new('P_InOther', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_InOther'])
bpy.data.objects['P_InOther'].location = bpy.data.objects['InOther'].location
bpy.data.objects.new('P_HideViewport', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_HideViewport'])
bpy.data.objects['P_HideViewport'].location = bpy.data.objects['HideViewport'].location
bpy.data.objects.new('P_InExcluded', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_InExcluded'])
bpy.data.objects['P_InExcluded'].location = bpy.data.objects['InExcluded'].location
bpy.context.scene.frame_current = 8
bpy.data.objects['Cube'].location = (7.0, 7.0, 7.0)
bpy.data.objects['InOther'].location = (7.0, 7.0, 7.0)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Light'].scale = (bpy.data.meshes['Plane'].shape_keys.key_blocks['K'].value, 1.0, 1.0)
bpy.data.objects['P_Cube'].rotation_euler = bpy.data.objects['Cube'].location
bpy.data.objects['P_InOther'].rotation_euler = bpy.data.objects['InOther'].location
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.data.objects.remove(bpy.data.objects['InOther'])
bpy.data.objects.remove(bpy.data.objects['HideViewport'])
bpy.data.objects.remove(bpy.data.objects['InExcluded'])

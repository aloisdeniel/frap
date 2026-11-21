# M5+M6 stabilization round 5: the viewport depsgraph builds an object operand of a modifier
# disabled for the viewport (`show_viewport = False`) invisible, so a session boundary does not
# animate it (`DepsgraphNodeBuilder::modifier_walk`); a texture of a disabled modifier is
# animated. Orphan operands of the Cube's modifiers are keyed (0,0,0)@1 -> (4,0,0)@11 (the
# textures' intensity 0@1 -> 1@11), frame_set(1), manual values, frame_set(6): the operands of
# the disabled BOOLEAN and MIRROR keep (9,9,9), those of the enabled ones move to (2,0,0), both
# textures read 0.5. Values are copied into probe empties; the orphans are removed before the
# dump (their matrix_world is stale in Blender, D-96).
# Round 6: an invisible operand keeps its time update pending. BoolRe (a cube of size 1, child
# of ParRe, both keyed and given manual values) is the operand of Host's disabled BOOLEAN at
# frame_set(6); `show_viewport = True` then `view_layer.update()` applies their animation at
# frame 6 (BoolRe (1,0,0), ParRe z 0.2) and the evaluated Host is cut (16 vertices, read by
# index: 8 before); toggling
# the modifier again after a manual value keeps it (no time change while invisible).
# golden: startup=factory tier=T1 tol=1e-5 compare=objects
import bpy
bpy.data.meshes.new('BM')
bpy.data.objects.new('BoolOff', bpy.data.meshes['BM'])
bpy.data.objects.new('BoolOn', bpy.data.meshes['BM'])
bpy.data.objects.new('MirrorOff', None)
bpy.data.objects.new('MirrorOn', None)
bpy.data.textures.new('TexOff', 'CLOUDS')
bpy.data.textures.new('TexOn', 'CLOUDS')
bpy.data.meshes['Cube'].copy()
bpy.data.objects.new('Host', bpy.data.meshes['Cube.001'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Host'])
bpy.data.objects.new('BoolRe', bpy.data.meshes['Cube.001'])
bpy.data.objects['BoolRe'].scale = (0.5, 0.5, 0.5)
bpy.data.objects.new('ParRe', None)
bpy.data.objects['BoolRe'].parent = bpy.data.objects['ParRe']
bpy.data.objects['Host'].modifiers.new('BRe', 'BOOLEAN')
bpy.data.objects['Host'].modifiers['BRe'].object = bpy.data.objects['BoolRe']
bpy.data.objects['Host'].modifiers['BRe'].show_viewport = False
bpy.data.objects['BoolRe'].keyframe_insert('location', frame=1)
bpy.data.objects['BoolRe'].location = (2.0, 0.0, 0.0)
bpy.data.objects['BoolRe'].keyframe_insert('location', frame=11)
bpy.data.objects['ParRe'].keyframe_insert('location', frame=1)
bpy.data.objects['ParRe'].location = (0.0, 0.0, 0.4)
bpy.data.objects['ParRe'].keyframe_insert('location', frame=11)
bpy.data.objects['Cube'].modifiers.new('BOff', 'BOOLEAN')
bpy.data.objects['Cube'].modifiers['BOff'].object = bpy.data.objects['BoolOff']
bpy.data.objects['Cube'].modifiers['BOff'].show_viewport = False
bpy.data.objects['Cube'].modifiers.new('BOn', 'BOOLEAN')
bpy.data.objects['Cube'].modifiers['BOn'].object = bpy.data.objects['BoolOn']
bpy.data.objects['Cube'].modifiers.new('MOff', 'MIRROR')
bpy.data.objects['Cube'].modifiers['MOff'].mirror_object = bpy.data.objects['MirrorOff']
bpy.data.objects['Cube'].modifiers['MOff'].show_viewport = False
bpy.data.objects['Cube'].modifiers.new('MOn', 'MIRROR')
bpy.data.objects['Cube'].modifiers['MOn'].mirror_object = bpy.data.objects['MirrorOn']
bpy.data.objects['Cube'].modifiers.new('DOff', 'DISPLACE')
bpy.data.objects['Cube'].modifiers['DOff'].texture = bpy.data.textures['TexOff']
bpy.data.objects['Cube'].modifiers['DOff'].show_viewport = False
bpy.data.objects['Cube'].modifiers.new('DOn', 'DISPLACE')
bpy.data.objects['Cube'].modifiers['DOn'].texture = bpy.data.textures['TexOn']
bpy.data.objects['BoolOff'].keyframe_insert('location', frame=1)
bpy.data.objects['BoolOff'].location = (4.0, 0.0, 0.0)
bpy.data.objects['BoolOff'].keyframe_insert('location', frame=11)
bpy.data.objects['BoolOn'].keyframe_insert('location', frame=1)
bpy.data.objects['BoolOn'].location = (4.0, 0.0, 0.0)
bpy.data.objects['BoolOn'].keyframe_insert('location', frame=11)
bpy.data.objects['MirrorOff'].keyframe_insert('location', frame=1)
bpy.data.objects['MirrorOff'].location = (4.0, 0.0, 0.0)
bpy.data.objects['MirrorOff'].keyframe_insert('location', frame=11)
bpy.data.objects['MirrorOn'].keyframe_insert('location', frame=1)
bpy.data.objects['MirrorOn'].location = (4.0, 0.0, 0.0)
bpy.data.objects['MirrorOn'].keyframe_insert('location', frame=11)
bpy.data.textures['TexOff'].intensity = 0.0
bpy.data.textures['TexOff'].keyframe_insert('intensity', frame=1)
bpy.data.textures['TexOff'].intensity = 1.0
bpy.data.textures['TexOff'].keyframe_insert('intensity', frame=11)
bpy.data.textures['TexOn'].intensity = 0.0
bpy.data.textures['TexOn'].keyframe_insert('intensity', frame=1)
bpy.data.textures['TexOn'].intensity = 1.0
bpy.data.textures['TexOn'].keyframe_insert('intensity', frame=11)
bpy.context.scene.frame_set(1)
bpy.data.objects['BoolOff'].location = (9.0, 9.0, 9.0)
bpy.data.objects['BoolOn'].location = (9.0, 9.0, 9.0)
bpy.data.objects['MirrorOff'].location = (9.0, 9.0, 9.0)
bpy.data.objects['MirrorOn'].location = (9.0, 9.0, 9.0)
bpy.data.textures['TexOff'].intensity = 0.9
bpy.data.textures['TexOn'].intensity = 0.9
bpy.data.objects['BoolRe'].location = (9.0, 9.0, 9.0)
bpy.data.objects['ParRe'].location = (9.0, 9.0, 9.0)
bpy.context.scene.frame_set(6)
bpy.data.objects.new('P_BoolOff', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_BoolOff'])
bpy.data.objects['P_BoolOff'].location = bpy.data.objects['BoolOff'].location
bpy.data.objects.new('P_BoolOn', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_BoolOn'])
bpy.data.objects['P_BoolOn'].location = bpy.data.objects['BoolOn'].location
bpy.data.objects.new('P_MirrorOff', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_MirrorOff'])
bpy.data.objects['P_MirrorOff'].location = bpy.data.objects['MirrorOff'].location
bpy.data.objects.new('P_MirrorOn', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_MirrorOn'])
bpy.data.objects['P_MirrorOn'].location = bpy.data.objects['MirrorOn'].location
bpy.data.objects.new('P_Tex', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Tex'])
bpy.data.objects['P_Tex'].scale = (bpy.data.textures['TexOff'].intensity, bpy.data.textures['TexOn'].intensity, 1.0)
bpy.data.objects.new('P_BoolReBefore', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_BoolReBefore'])
bpy.data.objects['P_BoolReBefore'].location = bpy.data.objects['BoolRe'].location
bpy.data.objects['P_BoolReBefore'].scale.x = bpy.data.objects['ParRe'].location.z
bpy.data.objects['Host'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.vertices[7].co
bpy.data.objects['Host'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.vertices[8].co
bpy.data.objects['Host'].modifiers['BRe'].show_viewport = True
bpy.context.view_layer.update()
bpy.data.objects.new('P_BoolReEnabled', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_BoolReEnabled'])
bpy.data.objects['P_BoolReEnabled'].location = bpy.data.objects['BoolRe'].location
bpy.data.objects['P_BoolReEnabled'].scale.x = bpy.data.objects['ParRe'].location.z
bpy.data.objects['Host'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.vertices[15].co
bpy.data.objects['Host'].evaluated_get(bpy.context.evaluated_depsgraph_get()).data.vertices[16].co
bpy.data.objects['Host'].modifiers['BRe'].show_viewport = False
bpy.context.view_layer.update()
bpy.data.objects['BoolRe'].location = (7.0, 7.0, 7.0)
bpy.data.objects['Host'].modifiers['BRe'].show_viewport = True
bpy.context.view_layer.update()
bpy.data.objects.new('P_BoolReToggled', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_BoolReToggled'])
bpy.data.objects['P_BoolReToggled'].location = bpy.data.objects['BoolRe'].location
bpy.data.objects['Host'].modifiers.clear()
bpy.data.objects.remove(bpy.data.objects['BoolRe'])
bpy.data.objects.remove(bpy.data.objects['ParRe'])
bpy.data.objects['Cube'].modifiers.clear()
bpy.data.objects.remove(bpy.data.objects['BoolOff'])
bpy.data.objects.remove(bpy.data.objects['BoolOn'])
bpy.data.objects.remove(bpy.data.objects['MirrorOff'])
bpy.data.objects.remove(bpy.data.objects['MirrorOn'])

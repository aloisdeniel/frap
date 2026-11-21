# WP 7.6: the 11 marker.* operators against Blender's in the factory Layout screen's Timeline
# (Blender needs an animation editor showing markers: the #@blender lines run each operator in a
# temp_override of the Timeline area; frap runs them headless-extended on the context scene's
# markers, OPERATORS.md). Covered: select_all (TOGGLE, INVERT, DESELECT, SELECT),
# select_leftright (LEFT, RIGHT + extend, CLICK_SIDE in exec), move, duplicate (copies at the
# head of the list), rename (first selected), select_box (region pixels of the Timeline's
# View2D: pixel 0 is frame -9, 1573 is 260), select (exact hits only headless; extend toggles;
# camera selects and activates the bound camera), camera_bind (new marker or the one on the
# frame; the scene camera follows), make_links_scene, add, delete. After each step a probe empty
# holds the marker frames (location, rotation_euler, scale.xy) and the selection bit mask
# (scale.z); reading the index past the end checks the count; probe parents hold the scene
# camera.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
#@blender TL = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender TL['region'] = [r for r in TL['area'].regions if r.type == 'WINDOW'][0]
bpy.context.scene.timeline_markers.new('A', frame=3)
bpy.context.scene.timeline_markers.new('B', frame=10)
bpy.context.scene.timeline_markers.new('C', frame=20)
#@frap bpy.ops.marker.select_all()
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_all()
bpy.context.scene.timeline_markers[3]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s01_toggle', None))
bpy.data.objects['s01_toggle'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s01_toggle'].rotation_euler = (0, 0, 0)
bpy.data.objects['s01_toggle'].scale = (0, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select)
#@frap bpy.ops.marker.select_all(action='INVERT')
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_all(action='INVERT')
bpy.context.scene.timeline_markers[3]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s02_invert', None))
bpy.data.objects['s02_invert'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s02_invert'].rotation_euler = (0, 0, 0)
bpy.data.objects['s02_invert'].scale = (0, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select)
#@frap bpy.ops.marker.select_all(action='DESELECT')
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_all(action='DESELECT')
bpy.context.scene.frame_current = 10
#@frap bpy.ops.marker.select_leftright(mode='LEFT')
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_leftright(mode='LEFT')
bpy.context.scene.timeline_markers[3]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s03_left', None))
bpy.data.objects['s03_left'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s03_left'].rotation_euler = (0, 0, 0)
bpy.data.objects['s03_left'].scale = (0, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select)
#@frap bpy.ops.marker.select_leftright(mode='RIGHT', extend=True)
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_leftright(mode='RIGHT', extend=True)
bpy.context.scene.timeline_markers[3]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s04_right', None))
bpy.data.objects['s04_right'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s04_right'].rotation_euler = (0, 0, 0)
bpy.data.objects['s04_right'].scale = (0, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select)
#@frap bpy.ops.marker.select_leftright(mode='CLICK_SIDE')
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_leftright(mode='CLICK_SIDE')
bpy.context.scene.timeline_markers[3]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s05_click_side', None))
bpy.data.objects['s05_click_side'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s05_click_side'].rotation_euler = (0, 0, 0)
bpy.data.objects['s05_click_side'].scale = (0, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select)
#@frap bpy.ops.marker.select_all(action='SELECT')
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_all(action='SELECT')
#@frap bpy.ops.marker.move(frames=2)
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.move(frames=2)
bpy.context.scene.timeline_markers[3]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s06_move', None))
bpy.data.objects['s06_move'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s06_move'].rotation_euler = (0, 0, 0)
bpy.data.objects['s06_move'].scale = (0, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select)
#@frap bpy.ops.marker.duplicate(frames=5)
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.duplicate(frames=5)
bpy.context.scene.timeline_markers[6]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s07_duplicate', None))
bpy.data.objects['s07_duplicate'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s07_duplicate'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s07_duplicate'].scale = (0, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select)
#@frap bpy.ops.marker.rename(name='Ren')
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.rename(name='Ren')
#@frap bpy.ops.marker.select_box(xmin=0, xmax=200)
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_box(xmin=0, xmax=200)
bpy.context.scene.timeline_markers[6]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s08_box', None))
bpy.data.objects['s08_box'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s08_box'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s08_box'].scale = (0, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select)
#@frap bpy.ops.marker.select_box(xmin=100, xmax=200, mode='SUB')
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_box(xmin=100, xmax=200, mode='SUB')
bpy.context.scene.timeline_markers[6]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s09_box_sub', None))
bpy.data.objects['s09_box_sub'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s09_box_sub'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s09_box_sub'].scale = (0, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select)
#@frap bpy.ops.marker.select_box(xmin=1573, xmax=0, mode='ADD')
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_box(xmin=1573, xmax=0, mode='ADD')
bpy.context.scene.timeline_markers[6]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s10_box_empty', None))
bpy.data.objects['s10_box_empty'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s10_box_empty'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s10_box_empty'].scale = (0, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select)
#@frap bpy.ops.marker.select_box()
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_box()
bpy.context.scene.timeline_markers[6]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s11_box_default', None))
bpy.data.objects['s11_box_default'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s11_box_default'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s11_box_default'].scale = (0, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select)
#@frap bpy.ops.marker.select(mouse_x=100)
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select(mouse_x=100)
bpy.context.scene.timeline_markers[6]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s12_select_miss', None))
bpy.data.objects['s12_select_miss'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s12_select_miss'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s12_select_miss'].scale = (0, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select)
bpy.context.scene.timeline_markers.new('Edge', frame=-9)
#@frap bpy.ops.marker.select_all(action='DESELECT')
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_all(action='DESELECT')
#@frap bpy.ops.marker.select(mouse_x=0)
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select(mouse_x=0)
bpy.context.scene.timeline_markers[7]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s13_select_hit', None))
bpy.data.objects['s13_select_hit'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s13_select_hit'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s13_select_hit'].scale = (bpy.context.scene.timeline_markers[6].frame, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select + 64 * bpy.context.scene.timeline_markers[6].select)
#@frap bpy.ops.marker.select(mouse_x=0, extend=True)
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select(mouse_x=0, extend=True)
bpy.context.scene.timeline_markers[7]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s14_select_extend', None))
bpy.data.objects['s14_select_extend'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s14_select_extend'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s14_select_extend'].scale = (bpy.context.scene.timeline_markers[6].frame, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select + 64 * bpy.context.scene.timeline_markers[6].select)
#@frap bpy.ops.marker.select(mouse_x=1573, extend=True)
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select(mouse_x=1573, extend=True)
bpy.context.scene.timeline_markers[7]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s15_select_extend_miss', None))
bpy.data.objects['s15_select_extend_miss'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s15_select_extend_miss'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s15_select_extend_miss'].scale = (bpy.context.scene.timeline_markers[6].frame, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select + 64 * bpy.context.scene.timeline_markers[6].select)
bpy.context.scene.frame_current = 30
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
#@frap bpy.ops.marker.camera_bind()
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.camera_bind()
bpy.context.scene.timeline_markers[8]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s16_bind_new', None))
bpy.data.objects['s16_bind_new'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s16_bind_new'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s16_bind_new'].scale = (bpy.context.scene.timeline_markers[6].frame, bpy.context.scene.timeline_markers[7].frame, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select + 64 * bpy.context.scene.timeline_markers[6].select + 128 * bpy.context.scene.timeline_markers[7].select)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('c16_bind_new', None))
bpy.data.objects['c16_bind_new'].parent = bpy.context.scene.camera
bpy.ops.object.camera_add(location=(0.0, -6.0, 1.0))
bpy.context.scene.frame_current = 10
#@frap bpy.ops.marker.camera_bind()
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.camera_bind()
bpy.context.scene.timeline_markers[8]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s17_bind_existing', None))
bpy.data.objects['s17_bind_existing'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s17_bind_existing'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s17_bind_existing'].scale = (bpy.context.scene.timeline_markers[6].frame, bpy.context.scene.timeline_markers[7].frame, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select + 64 * bpy.context.scene.timeline_markers[6].select + 128 * bpy.context.scene.timeline_markers[7].select)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('c17_bind_existing', None))
bpy.data.objects['c17_bind_existing'].parent = bpy.context.scene.camera
bpy.context.scene.camera = bpy.data.objects['Camera']
#@frap bpy.ops.marker.select_all(action='DESELECT')
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select_all(action='DESELECT')
bpy.context.scene.timeline_markers[2].select = True
#@frap bpy.ops.marker.move(frames=1)
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.move(frames=1)
bpy.context.scene.timeline_markers[8]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s18_move_switch', None))
bpy.data.objects['s18_move_switch'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s18_move_switch'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s18_move_switch'].scale = (bpy.context.scene.timeline_markers[6].frame, bpy.context.scene.timeline_markers[7].frame, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select + 64 * bpy.context.scene.timeline_markers[6].select + 128 * bpy.context.scene.timeline_markers[7].select)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('c18_move_switch', None))
bpy.data.objects['c18_move_switch'].parent = bpy.context.scene.camera
#@frap bpy.ops.marker.delete()
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.delete()
bpy.context.scene.timeline_markers[7]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s19_delete', None))
bpy.data.objects['s19_delete'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s19_delete'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s19_delete'].scale = (bpy.context.scene.timeline_markers[6].frame, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select + 64 * bpy.context.scene.timeline_markers[6].select)
bpy.data.scenes.new('S2')
bpy.context.scene.timeline_markers[6].select = True
#@frap bpy.ops.marker.make_links_scene(scene='S2')
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.make_links_scene(scene='S2')
bpy.context.scene.timeline_markers[7]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s20_links', None))
bpy.data.objects['s20_links'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s20_links'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s20_links'].scale = (bpy.context.scene.timeline_markers[6].frame, 0, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select + 64 * bpy.context.scene.timeline_markers[6].select)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('l20', None))
bpy.data.objects['l20'].location = (bpy.data.scenes['S2'].timeline_markers[0].frame, bpy.data.scenes['S2'].timeline_markers[0].select, 0)
bpy.data.objects['l20'].parent = bpy.data.scenes['S2'].timeline_markers[0].camera
bpy.data.scenes['S2'].timeline_markers[1]
bpy.context.scene.frame_current = 31
#@frap bpy.ops.marker.add()
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.add()
bpy.context.scene.timeline_markers[8]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s21_add', None))
bpy.data.objects['s21_add'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s21_add'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s21_add'].scale = (bpy.context.scene.timeline_markers[6].frame, bpy.context.scene.timeline_markers[7].frame, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select + 64 * bpy.context.scene.timeline_markers[6].select + 128 * bpy.context.scene.timeline_markers[7].select)
bpy.context.scene.timeline_markers[5].camera = bpy.data.objects['Camera.001']
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
#@frap bpy.ops.marker.select(mouse_x=0, camera=True)
#@blender with bpy.context.temp_override(**TL): bpy.ops.marker.select(mouse_x=0, camera=True)
bpy.context.scene.timeline_markers[8]
bpy.context.scene.collection.objects.link(bpy.data.objects.new('s22_select_camera', None))
bpy.data.objects['s22_select_camera'].location = (bpy.context.scene.timeline_markers[0].frame, bpy.context.scene.timeline_markers[1].frame, bpy.context.scene.timeline_markers[2].frame)
bpy.data.objects['s22_select_camera'].rotation_euler = (bpy.context.scene.timeline_markers[3].frame, bpy.context.scene.timeline_markers[4].frame, bpy.context.scene.timeline_markers[5].frame)
bpy.data.objects['s22_select_camera'].scale = (bpy.context.scene.timeline_markers[6].frame, bpy.context.scene.timeline_markers[7].frame, 1 * bpy.context.scene.timeline_markers[0].select + 2 * bpy.context.scene.timeline_markers[1].select + 4 * bpy.context.scene.timeline_markers[2].select + 8 * bpy.context.scene.timeline_markers[3].select + 16 * bpy.context.scene.timeline_markers[4].select + 32 * bpy.context.scene.timeline_markers[5].select + 64 * bpy.context.scene.timeline_markers[6].select + 128 * bpy.context.scene.timeline_markers[7].select)
bpy.context.scene.collection.objects.link(bpy.data.objects.new('n0', None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new('n1', None))
bpy.context.scene.collection.objects.link(bpy.data.objects.new('n7', None))
bpy.data.objects['n0'].name = bpy.context.scene.timeline_markers[0].name
bpy.data.objects['n1'].name = bpy.context.scene.timeline_markers[1].name
bpy.data.objects['n7'].name = bpy.context.scene.timeline_markers[7].name

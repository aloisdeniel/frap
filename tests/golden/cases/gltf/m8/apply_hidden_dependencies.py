# WP 8.12: `export_apply=True` evaluates a hidden object (`hide_viewport`) that an enabled
# modifier of an evaluated object pulls into the depsgraph: a Shrinkwrap target (a geometry
# relation) and an Array offset object (a transform relation: the add-on's
# `to_mesh(preserve_all_data_layers=True)` still evaluates its stack), so their Subdivision is
# applied in the export. The objects holding those modifiers are in `glTF_not_exported`, so
# only the hidden objects' meshes are compared. `build_object` walks every modifier's IDs
# (`BKE_modifiers_foreach_ID_link`), so a Boolean and an Array disabled in the viewport pull
# their hidden objects in too, and a Boolean pulls in both its object and its collection
# whatever its `operand_type`. (The Geometry Nodes Object Info case is a unit
# test of `apply.rs`: frap has no Object Info node.)
# golden: startup=factory tier=T2 compare=glb:out/deps.glb tol=1e-4 glb_structure
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_cube_add(size=1, location=(3.0, 0.0, 0.0))
bpy.context.active_object.name = 'Target'
bpy.data.objects['Target'].modifiers.new(name='Sub', type='SUBSURF')
bpy.data.objects['Target'].hide_viewport = True
bpy.ops.mesh.primitive_cube_add(size=1, location=(0.0, 3.0, 0.0))
bpy.context.active_object.name = 'Offset'
bpy.data.objects['Offset'].modifiers.new(name='Sub', type='SUBSURF')
bpy.data.objects['Offset'].hide_viewport = True
bpy.data.collections.new('glTF_not_exported')
bpy.context.scene.collection.children.link(bpy.data.collections['glTF_not_exported'])
bpy.ops.mesh.primitive_plane_add(location=(0.0, 0.0, 2.0))
bpy.context.active_object.name = 'Wrap'
bpy.data.objects['Wrap'].modifiers.new(name='Shrink', type='SHRINKWRAP')
bpy.data.objects['Wrap'].modifiers['Shrink'].target = bpy.data.objects['Target']
bpy.data.collections['glTF_not_exported'].objects.link(bpy.data.objects['Wrap'])
bpy.ops.mesh.primitive_plane_add(location=(0.0, 0.0, -4.0))
bpy.context.active_object.name = 'Arr'
bpy.data.objects['Arr'].modifiers.new(name='Arr', type='ARRAY')
bpy.data.objects['Arr'].modifiers['Arr'].use_object_offset = True
bpy.data.objects['Arr'].modifiers['Arr'].offset_object = bpy.data.objects['Offset']
bpy.data.collections['glTF_not_exported'].objects.link(bpy.data.objects['Arr'])
bpy.ops.mesh.primitive_cube_add(size=1, location=(6.0, 0.0, 0.0))
bpy.context.active_object.name = 'BoolOff'
bpy.data.objects['BoolOff'].modifiers.new(name='Sub', type='SUBSURF')
bpy.data.objects['BoolOff'].hide_viewport = True
bpy.ops.mesh.primitive_cube_add(size=1, location=(0.0, 6.0, 0.0))
bpy.context.active_object.name = 'ArrOff'
bpy.data.objects['ArrOff'].modifiers.new(name='Sub', type='SUBSURF')
bpy.data.objects['ArrOff'].hide_viewport = True
bpy.ops.mesh.primitive_cube_add(size=1, location=(9.0, 0.0, 0.0))
bpy.context.active_object.name = 'InColl'
bpy.data.objects['InColl'].modifiers.new(name='Sub', type='SUBSURF')
bpy.data.objects['InColl'].hide_viewport = True
bpy.ops.mesh.primitive_cube_add(size=1, location=(0.0, 9.0, 0.0))
bpy.context.active_object.name = 'InactiveObj'
bpy.data.objects['InactiveObj'].modifiers.new(name='Sub', type='SUBSURF')
bpy.data.objects['InactiveObj'].hide_viewport = True
bpy.data.collections.new('HiddenColl')
bpy.data.collections['HiddenColl'].objects.link(bpy.data.objects['InColl'])
bpy.ops.mesh.primitive_plane_add(location=(0.0, 0.0, 6.0))
bpy.context.active_object.name = 'OffBool'
bpy.data.objects['OffBool'].modifiers.new(name='B', type='BOOLEAN')
bpy.data.objects['OffBool'].modifiers['B'].object = bpy.data.objects['BoolOff']
bpy.data.objects['OffBool'].modifiers['B'].collection = bpy.data.collections['HiddenColl']
bpy.data.objects['OffBool'].modifiers['B'].show_viewport = False
bpy.data.collections['glTF_not_exported'].objects.link(bpy.data.objects['OffBool'])
bpy.ops.mesh.primitive_plane_add(location=(0.0, 0.0, 8.0))
bpy.context.active_object.name = 'OffArr'
bpy.data.objects['OffArr'].modifiers.new(name='A', type='ARRAY')
bpy.data.objects['OffArr'].modifiers['A'].use_object_offset = True
bpy.data.objects['OffArr'].modifiers['A'].offset_object = bpy.data.objects['ArrOff']
bpy.data.objects['OffArr'].modifiers['A'].show_viewport = False
bpy.data.collections['glTF_not_exported'].objects.link(bpy.data.objects['OffArr'])
bpy.ops.mesh.primitive_plane_add(location=(0.0, 0.0, 10.0))
bpy.context.active_object.name = 'CollBool'
bpy.data.objects['CollBool'].modifiers.new(name='B', type='BOOLEAN')
bpy.data.objects['CollBool'].modifiers['B'].operand_type = 'COLLECTION'
bpy.data.objects['CollBool'].modifiers['B'].object = bpy.data.objects['InactiveObj']
bpy.data.collections['glTF_not_exported'].objects.link(bpy.data.objects['CollBool'])
bpy.ops.export_scene.gltf(filepath='out/deps.glb', export_apply=True)

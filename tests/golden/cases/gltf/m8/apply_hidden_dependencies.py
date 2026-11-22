# WP 8.12: `export_apply=True` evaluates a hidden object (`hide_viewport`) that an enabled
# modifier of an evaluated object pulls into the depsgraph: a Shrinkwrap target (a geometry
# relation) and an Array offset object (a transform relation: the add-on's
# `to_mesh(preserve_all_data_layers=True)` still evaluates its stack), so their Subdivision is
# applied in the export. The objects holding those modifiers are in `glTF_not_exported`, so
# only the hidden objects' meshes are compared. (The Geometry Nodes Object Info case is a unit
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
bpy.ops.export_scene.gltf(filepath='out/deps.glb', export_apply=True)

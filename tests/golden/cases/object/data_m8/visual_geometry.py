# WP 8.3.6 (T1): object.visual_geometry_to_objects makes a mesh object of each selected mesh's
# evaluated mesh (modifiers applied; named after the source, Cube.001), with the source's local
# transform (as XYZ Euler), parent and collections, and the used materials on the data; an
# empty instancing a collection gives an empty instancing it (named after the collection); a
# light gives nothing; then only the new objects are selected, the first one active.
# golden: startup=factory compare=objects,context,mesh:Cube.001,mesh:Sphere.001 tier=T1 order=canonical
import bpy
bpy.ops.object.modifier_add(type='BEVEL')
bpy.context.object.modifiers['Bevel'].width = 0.25
bpy.ops.object.modifier_add(type='ARRAY')
bpy.context.object.rotation_mode = 'QUATERNION'
bpy.context.object.rotation_quaternion = (0.9238795, 0.0, 0.3826834, 0.0)
bpy.context.object.scale = (1.0, 2.0, 0.5)
bpy.context.object.delta_location = (0.0, 0.0, 1.0)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(0, 4, 0))
bpy.context.object.parent = bpy.data.objects['Camera']
bpy.ops.object.material_slot_add()
bpy.ops.object.material_slot_add()
bpy.context.object.material_slots[0].material = bpy.data.materials['Material']
bpy.data.collections.new('Bolts')
bpy.context.scene.collection.children.link(bpy.data.collections['Bolts'])
bpy.data.collections['Bolts'].objects.link(bpy.context.object)
bpy.ops.object.empty_add(location=(0, -4, 0), rotation=(0, 0, 0.5))
bpy.context.object.instance_type = 'COLLECTION'
bpy.context.object.instance_collection = bpy.data.collections['Bolts']
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Sphere'].select_set(True)
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.visual_geometry_to_objects()
bpy.data.objects['Cube.001'].rotation_mode
bpy.data.objects['Sphere.001'].parent
bpy.data.collections['Bolts'].objects['Sphere.001']
bpy.data.meshes['Sphere.001'].materials[0]
bpy.data.meshes['Sphere.001'].materials[1]
bpy.data.objects.new(bpy.data.objects['Bolts'].instance_collection.name, None)
bpy.data.objects['Bolts'].instance_type
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.visual_geometry_to_objects()
bpy.context.object

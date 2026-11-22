# WP 8.3.4: the instancing data behind collection_instance_add: Object.instance_type items follow
# the object type (an empty: NONE, COLLECTION; a mesh: NONE, VERTS, FACES), Object.
# instance_collection takes a collection only on an empty that is not inside it (else nothing
# changes, Blender only logs), None clears it, is_instancer is read-only, pass_index stays in
# its range; Collection.instance_offset. Values are read back as errors where they differ. (An
# unknown item lists the static items in frap, deviations §2.)
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.empty_add()
bpy.data.collections.new('C2')
bpy.data.objects['Empty'].instance_type = 'FACES'
bpy.data.objects['Cube'].instance_type = 'COLLECTION'
bpy.data.objects['Cube'].instance_type = 'VERTS'
bpy.data.objects['Cube'].instance_type = 'NONE'
bpy.data.objects['Empty'].instance_type = 'COLLECTION'
bpy.data.objects['Empty'].instance_collection.name
bpy.data.objects['Empty'].instance_collection = bpy.data.collections['Collection']
bpy.data.objects['Empty'].instance_collection.name
bpy.data.objects['Empty'].instance_collection = bpy.data.collections['C2']
bpy.data.objects['Empty'].instance_collection.name
bpy.data.objects['Cube'].instance_collection = bpy.data.collections['C2']
bpy.data.objects['Cube'].instance_collection.name
bpy.data.objects['Empty'].is_instancer = False
bpy.data.objects['Empty'].instance_type = 'NONE'
bpy.data.objects['Empty'].instance_collection.name
bpy.data.objects['Empty'].instance_collection = None
bpy.data.objects['Empty'].instance_collection.name
bpy.data.objects['Cube'].pass_index = -1
bpy.data.objects['Cube'].pass_index = 40000
bpy.data.objects['Cube'].pass_index = 7
bpy.data.collections['C2'].instance_offset = (1, 2, 3)
bpy.data.collections['C2'].instance_offset.x = 4
bpy.context.scene.collection.instance_offset = (0, 0, 1)
bpy.data.objects['Empty'].instance_type = 'COLLECTION'
bpy.data.objects['Empty'].instance_collection = bpy.data.collections['C2']
bpy.data.collections['C2'].objects.link(bpy.data.objects.new('Inside', None))
bpy.data.objects['Inside'].instance_collection = bpy.data.collections['C2']
bpy.data.objects['Inside'].instance_collection.name
bpy.ops.object.duplicates_make_real()
bpy.data.objects['Inside.001']
bpy.data.collections.remove(bpy.data.collections['C2'])

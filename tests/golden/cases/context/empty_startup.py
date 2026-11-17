# WP 1.26: the context of an empty startup (`wm.read_homefile(use_empty=True)`): no objects,
# no active object, nothing selected, Object Mode; the scene, view layer and scene collection
# exist. The first primitive becomes active and selected.
# golden: startup=empty tier=T0 compare=context,objects
import bpy
bpy.context.object
bpy.context.selected_objects
bpy.context.mode
bpy.context.scene.name
bpy.context.view_layer.name
bpy.context.collection.name
bpy.context.object.name
bpy.context.selected_objects[0]
bpy.data.objects[0]
bpy.ops.mesh.primitive_plane_add(size=4)
bpy.context.object.location.x = 1
bpy.context.collection.objects['Plane'].location.y = 2

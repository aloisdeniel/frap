# WP 1.26: errors raised by RNA functions and setters themselves (code RNA_RUNTIME), in
# Blender's wording ("Error: …" plus newline): an object outside the view layer cannot become
# active or selected, an ID with users is not removed without do_unlink, links are not doubled,
# a missing link is not unlinked, material indices out of range, unknown node types, bulk
# buffers of the wrong length, and path_from_id on an ID.
# golden: startup=factory tier=T0 compare=objects,materials,context
import bpy
bpy.data.objects.new('O', None)
bpy.context.view_layer.objects.active = bpy.data.objects['O']
bpy.data.objects['O'].select_set(True)
bpy.data.objects.remove(bpy.data.objects['Cube'], do_unlink=False)
bpy.data.collections['Collection'].objects.link(bpy.data.objects['Cube'])
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['O'])
bpy.data.objects['Cube'].data.materials.pop(index=5)
bpy.data.materials['Material'].node_tree.nodes.new('FooNode')
bpy.data.objects['Cube'].data.vertices.foreach_set('co', [1, 2])
bpy.data.objects['Cube'].data.vertices.foreach_get('co', [0, 0])
bpy.data.objects['Camera'].path_from_id()

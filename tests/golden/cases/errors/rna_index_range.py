# WP 1.26: indices out of range (code RNA_INDEX_RANGE, IndexError): ID collections, nameless
# mesh collections, slots and nodes (positive and negative), and math values (vector, matrix,
# array), on reads and writes, each with its own Blender wording.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects[9]
bpy.data.objects[-4]
bpy.data.meshes['Cube'].vertices[8]
bpy.data.meshes['Cube'].vertices[-9]
bpy.data.meshes['Cube'].polygons[6]
bpy.data.objects['Cube'].material_slots[1]
bpy.data.materials['Material'].node_tree.nodes[2]
bpy.data.objects['Cube'].location[3]
bpy.data.objects['Cube'].location[-4]
bpy.data.objects['Cube'].matrix_world[4]
bpy.data.objects['Cube'].matrix_world[0][4]
bpy.data.materials['Material'].diffuse_color[4]
bpy.data.objects['Cube'].location[3] = 1
bpy.data.meshes['Cube'].vertices[8].co = (0, 0, 0)
bpy.context.selected_objects[1]

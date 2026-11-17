# WP 1.20 (T0), object.join material merge: the active plane has no slot (one empty slot), the
# cube's effective slots (an object-linked 'Blue' included) are appended once each, every slot
# links DATA afterwards and the faces are remapped; two planes without slots join without any
# slot or material_index.
# golden: startup=factory compare=objects,context,mesh:Plane.001,mesh:Plane.002 tier=T0
import bpy
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.mesh.primitive_plane_add(location=(0, 0, 0))
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.data.objects['Plane'].select_set(True)
bpy.ops.object.join()
bpy.ops.object.select_all(action='DESELECT')
bpy.data.materials.new('Red')
bpy.data.materials.new('Blue')
bpy.ops.mesh.primitive_plane_add(location=(0, -3, 0))
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Red'])
bpy.data.meshes['Cube'].materials.append(None)
bpy.data.meshes['Cube'].polygons[2].material_index = 1
bpy.data.meshes['Cube'].polygons[3].material_index = 2
bpy.data.objects['Cube'].material_slots[1].link = 'OBJECT'
bpy.data.objects['Cube'].material_slots[1].material = bpy.data.materials['Blue']
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.join()
bpy.data.objects['Plane'].material_slots[2].link

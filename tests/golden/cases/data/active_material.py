# WP 1.22: `mesh.materials.pop/clear` clamp `active_material_index` of every object using the
# mesh (`BKE_object_material_resize` clamps `ob->actcol`); the `.name` reads fail on `None`, so
# they check which slot `active_material` reads, and `active_material = …` writes the clamped
# slot (dumped by `objects`).
# golden: startup=factory tier=T0 compare=objects,materials
import bpy
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('A'))
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('B'))
bpy.data.objects.new('Twin', bpy.data.meshes['Cube'])
bpy.data.objects['Cube'].active_material_index = 2
bpy.data.objects['Twin'].active_material_index = 2
bpy.data.meshes['Cube'].materials.pop()
bpy.data.objects['Cube'].active_material.name
bpy.data.objects['Twin'].active_material.name
bpy.data.meshes['Cube'].materials.pop()
bpy.data.objects['Cube'].active_material.name
bpy.data.objects['Cube'].active_material = bpy.data.materials['A']
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['B'])
bpy.data.objects['Cube'].active_material_index = 1
bpy.data.meshes['Cube'].materials.clear()
bpy.data.objects['Cube'].active_material.name
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['B'])
bpy.data.objects['Cube'].active_material.name
bpy.data.objects['Twin'].active_material = bpy.data.materials['A']
bpy.data.meshes['Cube'].materials.append(None)
bpy.data.objects['Cube'].active_material_index = 1
bpy.data.meshes['Cube'].materials.pop(index=0)
bpy.data.objects['Cube'].active_material
bpy.data.objects['Cube'].active_material = bpy.data.materials['B']

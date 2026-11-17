# WP 1.22: `mesh.materials.append/pop/clear`: slots follow on every object using the mesh;
# OBJECT-linked slot entries keep their position when slots are popped and are dropped when the
# list grows past the object's old slot count; `pop` shifts face `material_index` values;
# `clear` removes the attribute. `bpy.data.materials.new` gives the default node graph.
# golden: startup=factory tier=T0 compare=objects,materials,mesh:Cube
import bpy
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Red'))
bpy.data.meshes['Cube'].materials.append(None)
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Blue'))
bpy.data.meshes['Cube'].materials.append(bpy.data.objects['Cube'])
bpy.data.meshes['Cube'].materials.append()
bpy.data.objects.new('Twin', bpy.data.meshes['Cube'])
bpy.data.objects['Cube'].material_slots[1].link = 'OBJECT'
bpy.data.objects['Cube'].material_slots[1].material = bpy.data.materials['Blue']
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.data.meshes['Cube'].polygons[3].material_index = 3
bpy.data.meshes['Cube'].polygons[4].material_index = 3
bpy.data.meshes['Cube'].materials.pop(index=9)
bpy.data.meshes['Cube'].materials.pop(index=-5)
bpy.data.meshes['Cube'].materials.pop(-1)
bpy.data.meshes['Cube'].materials.pop(index=2)
bpy.data.meshes['Cube'].materials.pop(index=-1).name
bpy.data.meshes['Cube'].materials.pop(index=0).name
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Material'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Blue'])
bpy.data.meshes['Cube'].materials.clear(1)
bpy.data.meshes.new('Spare').materials.append(bpy.data.materials['Red'])
bpy.data.meshes['Spare'].materials.clear()
bpy.data.meshes['Spare'].materials.pop()
bpy.data.objects.new('SpareOb', bpy.data.meshes['Spare'])
bpy.data.meshes['Spare'].materials.append(bpy.data.materials['Material'])

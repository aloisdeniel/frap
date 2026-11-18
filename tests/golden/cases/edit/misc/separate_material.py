# WP 2.10: mesh.separate(type='MATERIAL'): the faces of the first face's material go to a new
# object per material until the last group, which the edit object keeps; each mesh then has
# only its own material (index 0).
# golden: startup=factory compare=objects,context,materials,mesh:Cube,mesh:Cube.001,mesh:Cube.002 tier=T1
import bpy
bpy.data.materials.new('Red')
bpy.data.materials.new('Blue')
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Red'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Blue'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Material'])
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[4].material_index = 1
bpy.data.meshes['Cube'].polygons[5].material_index = 2
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.separate(type='MATERIAL')
bpy.data.meshes['Cube'].materials[0].name
bpy.data.meshes['Cube.001'].materials[0].name
bpy.data.meshes['Cube.002'].materials[0].name

# WP 3.7: the life cycle of Mesh.calc_tangents()'s temporary tangent layer and the UV map
# lookup. The layer is dropped by leaving Edit Mode and is not part of mesh.copy() (both read
# zero tangents), while geometry edits, update() and removing the UV map keep the stale values.
# uvmap= reads any attribute converted to float2 on the corners (attributes.lookup<float2>): a
# POINT FLOAT2 attribute, 'position' (FLOAT_VECTOR) and a FACE FLOAT attribute work; only a
# missing name raises. Values are copied into object transforms so the objects dump compares
# them. (A failing call leaves zero tangents in Blender; frap rolls the statement back and
# keeps the previous ones: not compared.)
# golden: startup=factory tier=T1 compare=objects tol=1e-5
import bpy
bpy.data.meshes['Cube'].calc_tangents()
bpy.ops.object.editmode_toggle()
bpy.ops.object.editmode_toggle()
bpy.data.objects['Cube'].location = bpy.data.meshes['Cube'].loops[3].tangent
bpy.data.meshes['Cube'].calc_tangents()
bpy.data.meshes['Cube'].vertices[0].co = (5.0, 5.0, 5.0)
bpy.data.meshes['Cube'].update()
bpy.data.objects['Cube'].rotation_euler = bpy.data.meshes['Cube'].loops[3].tangent
bpy.data.meshes['Cube'].calc_tangents()
bpy.data.objects['Cube'].scale = bpy.data.meshes['Cube'].copy().loops[3].tangent
bpy.data.objects['Light'].location = bpy.data.meshes['Cube'].loops[3].tangent
bpy.data.meshes['Cube'].uv_layers.remove(bpy.data.meshes['Cube'].uv_layers['UVMap'])
bpy.data.objects['Light'].rotation_euler = bpy.data.meshes['Cube'].loops[3].tangent
bpy.data.meshes['Cube'].attributes.new('PF2', 'FLOAT2', 'POINT')
bpy.data.meshes['Cube'].attributes['PF2'].data[0].vector = (0.3, 0.9)
bpy.data.meshes['Cube'].attributes['PF2'].data[1].vector = (0.1, 0.2)
bpy.data.meshes['Cube'].calc_tangents(uvmap='PF2')
bpy.data.objects['Light'].scale = bpy.data.meshes['Cube'].loops[3].tangent
bpy.data.meshes['Cube'].calc_tangents(uvmap='position')
bpy.data.objects['Camera'].location = bpy.data.meshes['Cube'].loops[3].tangent
bpy.data.objects['Camera'].scale = bpy.data.meshes['Cube'].loops[5].bitangent
bpy.data.meshes['Cube'].attributes.new('F', 'FLOAT', 'FACE')
bpy.data.meshes['Cube'].attributes['F'].data.foreach_set('value', (0.1, 0.5, 0.9, 0.3, 0.7, 0.2))
bpy.data.meshes['Cube'].calc_tangents(uvmap='F')
bpy.data.objects['Camera'].rotation_euler = bpy.data.meshes['Cube'].loops[6].tangent
bpy.data.meshes['Cube'].calc_tangents(uvmap='nope')

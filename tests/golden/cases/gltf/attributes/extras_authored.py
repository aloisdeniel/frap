# M3+M4 stabilization round 2: custom properties authored in both tools (`ID[key] = value`,
# ID properties on `IdHeader.custom`) become glTF `extras` with export_extras: scalars,
# strings, int and float arrays, bool lists, string lists and nested groups, on the object,
# its mesh, its material and the scene; read back by subscript and `get()`, then
# re-assigned.
# golden: startup=factory tier=T2 compare=glb:out/extras.glb glb_structure
import bpy
bpy.data.objects['Cube']['tag'] = 'hero'
bpy.data.objects['Cube']['count'] = 3
bpy.data.objects['Cube']['weight'] = 0.25
bpy.data.objects['Cube']['flag'] = True
bpy.data.objects['Cube']['ints'] = [1, 2, 3]
bpy.data.objects['Cube']['floats'] = [1, 2.5]
bpy.data.objects['Cube']['bools'] = [True, False]
bpy.data.objects['Cube']['names'] = ['a', 'b']
bpy.data.objects['Cube']['group'] = {'k': 1, 'inner': {'x': 'y'}}
bpy.data.objects['Cube']['offset'] = bpy.data.objects['Cube'].scale
bpy.data.objects['Cube']['count'] += 2
bpy.data.objects['Cube']['tag']
bpy.data.objects['Cube']['ints'][1]
bpy.data.objects['Cube']['group']['inner']['x']
bpy.data.objects['Cube'].get('missing', 7)
bpy.data.objects['Cube']['missing']
bpy.data.objects['Cube']['big'] = [1, 'a']
bpy.data.meshes['Cube']['mesh_prop'] = 'm'
bpy.data.materials['Material']['mat_prop'] = 7
bpy.data.scenes['Scene']['scene_prop'] = 'scene'
bpy.data.meshes['Cube'].vertices[0]['x']
bpy.ops.export_scene.gltf(filepath='out/extras.glb', export_extras=True)

# WP 3.7: export_tangents=True writes MikkTSpace TANGENT (w = bitangent sign) from the active UV
# map, rounded to 4 decimals and Y-up: a flat cube, a smooth UV sphere, a cylinder whose UVs
# are mirrored on half the faces (w = -1 there), Z-up (export_yup=False), and, without normals
# (export_normals=False), no tangents at all. A mesh with an n-gon cannot get tangents: it
# exports without TANGENT.
# golden: startup=factory tier=T2 compare=glb:out/tangents.glb,glb:out/no_normals.glb,glb:out/zup.glb,glb:out/ngon.glb tol=1e-4 glb_structure
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, location=(3, 0, 0))
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(-3, 0, 0))
bpy.data.meshes['Cylinder'].uv_layers['UVMap'].data[0].uv = (0.9, 0.1)
bpy.data.meshes['Cylinder'].uv_layers['UVMap'].data[1].uv = (0.1, 0.1)
bpy.data.meshes['Cylinder'].uv_layers['UVMap'].data[2].uv = (0.1, 0.9)
bpy.data.meshes['Cylinder'].uv_layers['UVMap'].data[3].uv = (0.9, 0.9)
bpy.data.meshes['Cylinder'].uv_layers.new(name='Second')
bpy.data.meshes['Cylinder'].uv_layers.active_index = 1
bpy.data.meshes['Cylinder'].uv_layers['Second'].data[4].uv = (0.3, 0.7)
bpy.ops.export_scene.gltf(filepath='out/tangents.glb', export_tangents=True)
bpy.ops.export_scene.gltf(filepath='out/no_normals.glb', export_tangents=True, export_normals=False)
bpy.ops.export_scene.gltf(filepath='out/zup.glb', export_tangents=True, export_yup=False)
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.mesh.primitive_circle_add(vertices=6, fill_type='NGON', location=(0, 4, 0))
bpy.ops.export_scene.gltf(filepath='out/ngon.glb', export_tangents=True, use_selection=True)

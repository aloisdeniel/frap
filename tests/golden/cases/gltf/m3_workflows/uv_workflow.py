# WP 3.9: an agent UV-maps a cube for a texture atlas: it marks the standard cube seams in Edit
# Mode, unwraps CONFORMAL (LSCM, equal at the default 1e-5), adds a second UV map filled by
# cube_project and exports both maps (TEXCOORD_0 and TEXCOORD_1) with a checker texture.
# A cylinder gets cylinder_project and a sphere sphere_project. Round trip (§17.2).
# golden: startup=factory tier=T2 compare=glb:out/atlas.glb,glb:out/atlas_rt.glb glb_structure
import bpy
bpy.ops.image.new(name='Checker', width=8, height=8, generated_type='COLOR_GRID')
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Checker']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
#@frap    bpy.ops.frap.select_elements(type='EDGE', indices=[0, 2, 3, 5, 6, 9, 11])
#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); bm.edges.ensure_lookup_table(); [bm.edges[i].select_set(True) for i in (0, 2, 3, 5, 6, 9, 11)]; bmesh.update_edit_mesh(bpy.context.object.data)
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='CONFORMAL', fill_holes=False, margin=0.02)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers.new(name='Lightmap')
bpy.data.meshes['Cube'].uv_layers.active_index = 1
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.cube_project(cube_size=2.0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.cylinder_project()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(-3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.sphere_project()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.export_scene.gltf(filepath='out/atlas.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/atlas.glb')
bpy.ops.export_scene.gltf(filepath='out/atlas_rt.glb')

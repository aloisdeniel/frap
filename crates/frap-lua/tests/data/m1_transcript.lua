-- SPDX-License-Identifier: GPL-3.0-or-later
-- WP 8.10: the M1 exit transcript (ARCHITECTURE §22.3, tests/cli/m1/vertical_slice.in/
-- vertical_slice.py) re-expressed in Lua, statement for statement. Output paths are
-- cwd-relative, as in the transcript.
--
-- The render needs a GPU adapter (frap-lua's `gpu` feature): without one both front-ends
-- answer GPU_UNAVAILABLE at that statement, so it is the one statement run under pcall.
bpy.ops.object.select_all{action='SELECT'}
bpy.ops.object.delete()
bpy.ops.mesh.primitive_cylinder_add{vertices=16, radius=0.5, depth=2, location={0, 0, 1}}
bpy.data.materials.new("Steel")
bpy.data.materials['Steel'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Steel'].diffuse_color = {0.6, 0.6, 0.7, 1.0}
bpy.context.object.data.materials:append(bpy.data.materials['Steel'])
bpy.ops.object.camera_add{location={4, -4, 3}, rotation={1.1, 0, 0.785}}
bpy.context.scene.camera = bpy.data.objects['Camera']
bpy.data.scenes['Scene'].render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.render.filepath = 'out/part.png'
local ok, err = pcall(bpy.ops.render.render, {write_still=true})
if not ok and err.code ~= 'GPU_UNAVAILABLE' then
    error(err)
end
rendered = ok and 1 or 0
bpy.ops.export_scene.gltf{filepath='out/part.glb'}
bpy.ops.wm.save_as_mainfile{filepath='out/part.frap'}
bpy.ops.wm.read_homefile{use_empty=true}
bpy.ops.import_scene.gltf{filepath='out/part.glb'}
imported_metallic = bpy.data.objects['Cylinder'].data.materials[0].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value
bpy.ops.wm.open_mainfile{filepath='out/part.frap'}
cylinder_location = frap.value(bpy.data.objects['Cylinder'].location)

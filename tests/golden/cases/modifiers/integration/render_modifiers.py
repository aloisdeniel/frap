# golden: startup=factory compare=objects,context,materials tier=T0
# render-golden: image=out/render_modifiers.png min_ssim=0.97
# Render golden (WP 4.7): renders show modifiers, evaluated for the render: the cube's
# Subdivision uses render_levels (2) while its viewport level is 0; the sphere's Wireframe is
# on in the viewport but off in renders (show_render=False), so it renders solid; the plane's
# Solidify puts its rim on a second material through material_offset_rim (Workbench MATERIAL
# colours); 'Twin', a linked duplicate of the cube with an Array of its own, renders
# its own geometry while sharing the data.
import bpy
bpy.data.materials['Material'].diffuse_color = (0.8, 0.3, 0.1, 1.0)
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube'].modifiers['Subdivision'].levels = 0
bpy.data.objects['Cube'].modifiers['Subdivision'].render_levels = 2
bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8, radius=0.8, location=(2.6, 0.5, 0.2))
bpy.data.objects['Sphere'].modifiers.new(name='Wireframe', type='WIREFRAME')
bpy.data.objects['Sphere'].modifiers['Wireframe'].show_render = False
bpy.ops.mesh.primitive_plane_add(size=2, location=(0.2, 2.8, -0.6), rotation=(0.5, 0.2, 0))
bpy.data.materials.new('Teal')
bpy.data.materials['Teal'].diffuse_color = (0.1, 0.6, 0.6, 1.0)
bpy.data.materials.new('Yellow')
bpy.data.materials['Yellow'].diffuse_color = (0.9, 0.8, 0.1, 1.0)
bpy.data.objects['Plane'].data.materials.append(bpy.data.materials['Teal'])
bpy.data.objects['Plane'].data.materials.append(bpy.data.materials['Yellow'])
bpy.data.objects['Plane'].modifiers.new(name='Solidify', type='SOLIDIFY')
bpy.data.objects['Plane'].modifiers['Solidify'].thickness = 0.4
bpy.data.objects['Plane'].modifiers['Solidify'].material_offset_rim = 1
bpy.data.objects.new('Twin', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Twin'])
bpy.data.objects['Twin'].location = (-2.6, -1.0, 0.0)
bpy.data.objects['Twin'].scale = (0.4, 0.4, 0.4)
bpy.data.objects['Twin'].modifiers.new(name='Array', type='ARRAY')
bpy.data.objects['Twin'].modifiers['Array'].count = 3
bpy.context.scene.render.engine = 'BLENDER_WORKBENCH'
bpy.context.scene.display.shading.color_type = 'MATERIAL'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.render.filepath = 'out/render_modifiers'
#@render bpy.ops.render.render(write_still=True)

# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/transmission.png min_ssim=0.85
# EEVEE shading feature golden (WP 5.5): transmission. A clear glass sphere (Transmission
# Weight 1, Roughness 0.05, IOR 1.45) and a frosted green one (Roughness 0.4) in front of a
# red wall, under a blue world. Without ray-traced refraction (Blender's default) the
# refraction closure sees the world probe, not the wall behind.
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_plane_add(size=8.0, location=(0.0, 2.0, 0.0), rotation=(1.5708, 0.0, 0.0))
bpy.data.materials.new('Wall')
bpy.data.materials['Wall'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.05, 0.05, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Wall'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.9, location=(-1.1, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Clear')
bpy.data.materials['Clear'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['Clear'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.05
bpy.data.materials['Clear'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 1.45
bpy.data.materials['Clear'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1.0, 1.0, 1.0, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Clear'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.9, location=(1.1, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Frosted')
bpy.data.materials['Frosted'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['Frosted'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.4
bpy.data.materials['Frosted'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.3, 0.9, 0.4, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Frosted'])
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.2, 0.3, 0.6, 1.0)
bpy.data.objects['Camera'].location = (0.0, -6.0, 0.5)
bpy.data.objects['Camera'].rotation_euler = (1.49, 0.0, 0.0)
bpy.data.objects['Light'].location = (3.0, -4.0, 4.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 160
bpy.context.scene.render.resolution_y = 120
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/transmission'
#@render bpy.ops.render.render(write_still=True)

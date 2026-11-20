# golden: startup=factory compare=objects tier=T0
# render-golden: image=out/sheen.png min_ssim=0.95 max_delta_e=0.6
# EEVEE shading feature golden (WP 5.5): sheen. Dark blue velvet-like spheres (Base Color
# 0.02, 0.03, 0.1, Roughness 1) with a pale pink Sheen Tint, Sheen Weight 1 and 0.5, Sheen
# Roughness 0.2 and 0.8, next to the same sphere without sheen: the sheen brightens the
# grazing angles.
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.8, location=(-1.9, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Plain')
bpy.data.materials['Plain'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.02, 0.03, 0.1, 1.0)
bpy.data.materials['Plain'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 1.0
bpy.context.object.data.materials.append(bpy.data.materials['Plain'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.8, location=(0.0, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Sheen2')
bpy.data.materials['Sheen2'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.02, 0.03, 0.1, 1.0)
bpy.data.materials['Sheen2'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 1.0
bpy.data.materials['Sheen2'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 1.0
bpy.data.materials['Sheen2'].node_tree.nodes['Principled BSDF'].inputs['Sheen Roughness'].default_value = 0.2
bpy.data.materials['Sheen2'].node_tree.nodes['Principled BSDF'].inputs['Sheen Tint'].default_value = (1.0, 0.7, 0.8, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Sheen2'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=48, ring_count=24, radius=0.8, location=(1.9, 0.0, 0.0))
bpy.ops.object.shade_smooth()
bpy.data.materials.new('Sheen8')
bpy.data.materials['Sheen8'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.02, 0.03, 0.1, 1.0)
bpy.data.materials['Sheen8'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 1.0
bpy.data.materials['Sheen8'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 0.5
bpy.data.materials['Sheen8'].node_tree.nodes['Principled BSDF'].inputs['Sheen Roughness'].default_value = 0.8
bpy.data.materials['Sheen8'].node_tree.nodes['Principled BSDF'].inputs['Sheen Tint'].default_value = (1.0, 0.7, 0.8, 1.0)
bpy.context.object.data.materials.append(bpy.data.materials['Sheen8'])
bpy.data.worlds['World'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.25, 0.25, 0.25, 1.0)
bpy.data.objects['Camera'].location = (0.0, -6.5, 1.5)
bpy.data.objects['Camera'].rotation_euler = (1.35, 0.0, 0.0)
bpy.data.objects['Light'].location = (0.0, 3.0, 3.0)
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 200
bpy.context.scene.render.resolution_y = 100
bpy.context.scene.view_settings.view_transform = 'Standard'
bpy.context.scene.eevee.taa_render_samples = 16
bpy.context.scene.render.filepath = 'out/sheen'
#@render bpy.ops.render.render(write_still=True)

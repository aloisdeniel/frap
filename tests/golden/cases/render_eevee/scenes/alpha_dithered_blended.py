# golden: startup=factory compare=objects,materials,context tier=T0
# render-golden: image=out/alpha_dithered_blended.png
# M5 exit scene (WP 5.6): transparency over an opaque scene: a red plane with Alpha 0.4 rendered
# DITHERED (Blender's default: TAA averages the hashed samples) and a blue plane with Alpha 0.6
# rendered BLENDED, both in front of a white cube on a floor, factory light, AgX. Both cast
# transparent shadows (`use_transparent_shadow`, Blender's default; WP 5.5).
# Dither off (`render.dither_intensity = 0`); `night_low_contrast_dither.py` says why.
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_cube_add(size=1.5, location=(0.0, 1.0, 0.75))
bpy.data.materials.new('White')
bpy.data.materials['White'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.8, 0.8, 1.0)
bpy.data.materials['White'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.5
bpy.context.object.data.materials.append(bpy.data.materials['White'])
bpy.data.materials.new('Floor')
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.3, 0.3, 0.3, 1.0)
bpy.data.materials['Floor'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.8
bpy.ops.mesh.primitive_plane_add(size=20.0, location=(0.0, 0.0, 0.0))
bpy.context.object.data.materials.append(bpy.data.materials['Floor'])
bpy.ops.mesh.primitive_plane_add(size=1.6, location=(-0.6, -0.6, 1.0), rotation=(1.5707963, 0.0, 0.8))
bpy.data.materials.new('RedGlass')
bpy.data.materials['RedGlass'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.05, 0.05, 1.0)
bpy.data.materials['RedGlass'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.4
bpy.data.materials['RedGlass'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value = 0.4
bpy.context.object.data.materials.append(bpy.data.materials['RedGlass'])
bpy.ops.mesh.primitive_plane_add(size=1.6, location=(0.9, -0.9, 1.0), rotation=(1.5707963, 0.0, 0.8))
bpy.data.materials.new('BlueGlass')
bpy.data.materials['BlueGlass'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.05, 0.1, 0.8, 1.0)
bpy.data.materials['BlueGlass'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.4
bpy.data.materials['BlueGlass'].node_tree.nodes['Principled BSDF'].inputs['Alpha'].default_value = 0.6
bpy.data.materials['BlueGlass'].surface_render_method = 'BLENDED'
bpy.context.object.data.materials.append(bpy.data.materials['BlueGlass'])
bpy.context.scene.render.engine = 'BLENDER_EEVEE'
bpy.context.scene.render.resolution_x = 320
bpy.context.scene.render.resolution_y = 180
bpy.context.scene.eevee.taa_render_samples = 32
bpy.context.scene.render.dither_intensity = 0.0
bpy.context.scene.render.filepath = 'out/alpha_dithered_blended'
#@render bpy.ops.render.render(write_still=True)

# WP 3.4: re-typing the factory light exposes the values startup.blend stores for the other
# types (75 degree spot cone, 11.4 degree sun angle, 0.1 x 0.1 rectangle area, cascade
# distance 1000), here copied into object transforms and exported as a spot and as a sun.
# golden: startup=factory tier=T2 compare=objects,glb:out/spot.glb,glb:out/sun.glb
import bpy
bpy.data.lights['Light'].type = 'SPOT'
bpy.data.objects['Cube'].location = (bpy.data.lights['Light'].spot_size, bpy.data.lights['Light'].spot_blend, bpy.data.lights['Light'].shadow_soft_size)
bpy.ops.export_scene.gltf(filepath='out/spot.glb', export_lights=True)
bpy.data.lights['Light'].type = 'SUN'
bpy.data.objects['Cube'].scale = (bpy.data.lights['Light'].angle, bpy.data.lights['Light'].shadow_cascade_max_distance, bpy.data.lights['Light'].energy)
bpy.data.lights['Light'].normalize = False
bpy.ops.export_scene.gltf(filepath='out/sun.glb', export_lights=True)
bpy.data.lights['Light'].type = 'AREA'
bpy.data.objects['Camera'].location = (bpy.data.lights['Light'].size, bpy.data.lights['Light'].size_y, bpy.data.lights['Light'].spread)

# WP 1.26: coercion and clamping on write (§9.2; the frap INFO reports of D-03 are not
# compared): ints and bools for floats, 0/1 for booleans, values beyond the hard range clamped
# (material factors, render resolution and percentage, camera lens, unit scale), negative
# values kept where there is no minimum. Each result is moved into an object
# transform so that it is compared.
# golden: startup=factory tier=T0 compare=objects,materials
import bpy
bpy.data.materials['Material'].roughness = 2
bpy.data.materials['Material'].metallic = -1
bpy.data.scenes['Scene'].render.resolution_x = 2
bpy.data.scenes['Scene'].render.resolution_percentage = 0
bpy.data.lights['Light'].energy = -5
bpy.data.cameras['Camera'].lens = 0
bpy.data.scenes['Scene'].unit_settings.scale_length = 0
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = True
bpy.data.objects['Cube'].hide_render = 1
bpy.data.objects['Cube'].scale = (True, 2, 3)
bpy.data.objects['Cube'].location = (bpy.data.materials['Material'].roughness, bpy.data.materials['Material'].metallic, bpy.data.lights['Light'].energy)
bpy.data.objects['Light'].location = (bpy.data.scenes['Scene'].render.resolution_x, bpy.data.scenes['Scene'].render.resolution_percentage, bpy.data.cameras['Camera'].lens)
bpy.data.objects['Light'].scale.x = bpy.data.scenes['Scene'].unit_settings.scale_length
bpy.data.objects['Light'].scale.y = bpy.data.objects['Cube'].hide_render

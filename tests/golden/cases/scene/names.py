# WP 1.10: `ID.name` assignment: a taken name gives the assigned ID the lowest free `.NNN`; an
# empty name gives the type's default name; each `bpy.data` collection is its own namespace.
# golden: startup=factory tier=T0 compare=objects,materials
import bpy
bpy.data.objects['Cube'].name = 'Camera'
bpy.data.objects['Light'].name = ''
bpy.data.materials['Material'].name = 'Camera'
bpy.data.objects['Camera.001'].name = 'Camera.001'

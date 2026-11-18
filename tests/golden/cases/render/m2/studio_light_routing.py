# golden: startup=factory compare=objects,context tier=T0
# WP 2.3: `View3DShading.studio_light` lists only the lights of the current `type`/`light`
# (`rna_View3DShading_studio_light_itemf`): studio rigs in SOLID + STUDIO/FLAT, matcaps in
# SOLID + MATCAP, world HDRIs in MATERIAL/RENDERED, nothing in WIREFRAME. Every refused
# assignment is Blender's enum TypeError listing the current items; the valid ones land in the
# routed DNA slot.
import bpy
bpy.context.scene.display.shading.studio_light = 'clay_brown.exr'
bpy.context.scene.display.shading.studio_light = 'rim.sl'
bpy.context.scene.display.shading.light = 'MATCAP'
bpy.context.scene.display.shading.studio_light = 'rim.sl'
bpy.context.scene.display.shading.studio_light = 'clay_brown.exr'
bpy.context.scene.display.shading.light = 'FLAT'
bpy.context.scene.display.shading.studio_light = 'clay_brown.exr'
bpy.context.scene.display.shading.type = 'MATERIAL'
bpy.context.scene.display.shading.studio_light = 'Default'
bpy.context.scene.display.shading.studio_light = 'city.exr'
bpy.context.scene.display.shading.type = 'WIREFRAME'
bpy.context.scene.display.shading.studio_light = 'forest.exr'
bpy.context.scene.display.shading.type = 'SOLID'
bpy.context.scene.display.shading.studio_light = 'paint.sl'

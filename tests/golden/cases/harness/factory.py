# Seed case of the golden harness (WP 1.8): the factory scene after a property write, with a
# line each tool runs on its own (`#@frap` / `#@blender`).
# golden: startup=factory compare=objects,mesh:Cube,context tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].location = (0.5, -0.25, 2)
#@frap    bpy.data.objects['Cube'].location
#@blender bpy.context.view_layer.update()
bpy.data.objects['Camera'].location

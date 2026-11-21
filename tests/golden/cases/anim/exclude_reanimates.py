# M5+M6 stabilization round 5: a write of `LayerCollection.exclude` that leaves the layer
# collection included tags every object of the view layer for an animation update
# (`rna_LayerCollection_exclude_update`), so the next boundary re-applies the animation at the
# current frame. The Cube is keyed (0,0,0)@1 -> (4,0,0)@11; each step sets a manual location
# and copies the result of the next `view_layer.update()` into a probe empty: re-including the
# Cube's own collection, re-including an unrelated empty collection and writing
# `exclude = False` on an included one re-animate it (2,0,0); toggling a layer collection's or
# a collection's `hide_viewport`, the object's `hide_viewport`, or excluding a collection does
# not.
# golden: startup=factory tier=T1 tol=1e-5 compare=objects
import bpy
bpy.data.collections.new('Other')
bpy.context.scene.collection.children.link(bpy.data.collections['Other'])
bpy.data.objects['Cube'].location = (0.0, 0.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube'].location = (4.0, 0.0, 0.0)
bpy.data.objects['Cube'].keyframe_insert('location', frame=11)
bpy.context.view_layer.layer_collection.children['Collection'].exclude = True
bpy.context.scene.frame_set(6)
bpy.data.objects['Cube'].location = (9.0, 9.0, 9.0)
bpy.context.view_layer.layer_collection.children['Collection'].exclude = False
bpy.context.view_layer.update()
bpy.data.objects.new('P_Reinclude', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Reinclude'])
bpy.data.objects['P_Reinclude'].location = bpy.data.objects['Cube'].location
bpy.data.objects['Cube'].location = (9.0, 9.0, 9.0)
bpy.context.view_layer.layer_collection.children['Other'].exclude = True
bpy.context.view_layer.update()
bpy.data.objects.new('P_ExcludeOther', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_ExcludeOther'])
bpy.data.objects['P_ExcludeOther'].location = bpy.data.objects['Cube'].location
bpy.context.view_layer.layer_collection.children['Other'].exclude = False
bpy.context.view_layer.update()
bpy.data.objects.new('P_ReincludeOther', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_ReincludeOther'])
bpy.data.objects['P_ReincludeOther'].location = bpy.data.objects['Cube'].location
bpy.data.objects['Cube'].location = (8.0, 8.0, 8.0)
bpy.context.view_layer.layer_collection.children['Collection'].hide_viewport = True
bpy.context.view_layer.layer_collection.children['Collection'].hide_viewport = False
bpy.data.collections['Collection'].hide_viewport = True
bpy.data.collections['Collection'].hide_viewport = False
bpy.data.objects['Cube'].hide_viewport = True
bpy.data.objects['Cube'].hide_viewport = False
bpy.context.view_layer.update()
bpy.data.objects.new('P_HideToggles', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_HideToggles'])
bpy.data.objects['P_HideToggles'].location = bpy.data.objects['Cube'].location
bpy.data.objects['Cube'].location = (7.0, 7.0, 7.0)
bpy.context.view_layer.layer_collection.children['Collection'].exclude = False
bpy.context.view_layer.update()
bpy.data.objects.new('P_IncludedAgain', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_IncludedAgain'])
bpy.data.objects['P_IncludedAgain'].location = bpy.data.objects['Cube'].location
bpy.data.objects['Cube'].location = (6.0, 6.0, 6.0)

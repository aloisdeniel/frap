# M5+M6 stabilization round 6: `LayerCollection.exclude` sets the whole subtree
# (`rna_LayerCollection_exclude_update_recursive`, with `LAYER_COLLECTION_PREVIOUSLY_EXCLUDED`).
# Collection holds Sub (holding Deep) and Sub2. Each probe's location holds the exclude flags
# of (Sub, Sub2, Deep) after a step: excluding Collection excludes all three and re-including
# it includes them again; a Sub excluded before stays excluded (with Deep), also after a
# no-op `exclude = False` on the included Collection; a second `exclude = True` remembers
# every child as excluded (Sub2 stays excluded from then on); a Deep included under an
# excluded Sub is re-included.
# golden: startup=factory compare=objects tier=T1 tol=1e-6
import bpy
bpy.data.collections.new('Sub')
bpy.data.collections['Collection'].children.link(bpy.data.collections['Sub'])
bpy.data.collections.new('Sub2')
bpy.data.collections['Collection'].children.link(bpy.data.collections['Sub2'])
bpy.data.collections.new('Deep')
bpy.data.collections['Sub'].children.link(bpy.data.collections['Deep'])
bpy.context.view_layer.layer_collection.children['Collection'].exclude = True
bpy.data.objects.new('P_Excluded', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Excluded'])
bpy.data.objects['P_Excluded'].location = (bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub2'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].children['Deep'].exclude)
bpy.context.view_layer.layer_collection.children['Collection'].exclude = False
bpy.data.objects.new('P_Included', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_Included'])
bpy.data.objects['P_Included'].location = (bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub2'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].children['Deep'].exclude)
bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].exclude = True
bpy.data.objects.new('P_SubExcluded', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_SubExcluded'])
bpy.data.objects['P_SubExcluded'].location = (bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub2'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].children['Deep'].exclude)
bpy.context.view_layer.layer_collection.children['Collection'].exclude = True
bpy.context.view_layer.layer_collection.children['Collection'].exclude = False
bpy.data.objects.new('P_SubKept', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_SubKept'])
bpy.data.objects['P_SubKept'].location = (bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub2'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].children['Deep'].exclude)
bpy.context.view_layer.layer_collection.children['Collection'].exclude = False
bpy.data.objects.new('P_IncludeIncluded', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_IncludeIncluded'])
bpy.data.objects['P_IncludeIncluded'].location = (bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub2'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].children['Deep'].exclude)
bpy.context.view_layer.layer_collection.children['Collection'].exclude = True
bpy.context.view_layer.layer_collection.children['Collection'].exclude = True
bpy.context.view_layer.layer_collection.children['Collection'].exclude = False
bpy.data.objects.new('P_ExcludedTwice', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_ExcludedTwice'])
bpy.data.objects['P_ExcludedTwice'].location = (bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub2'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].children['Deep'].exclude)
bpy.context.view_layer.layer_collection.children['Collection'].exclude = False
bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].exclude = True
bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].children['Deep'].exclude = False
bpy.context.view_layer.layer_collection.children['Collection'].exclude = True
bpy.context.view_layer.layer_collection.children['Collection'].exclude = False
bpy.data.objects.new('P_DeepIncluded', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['P_DeepIncluded'])
bpy.data.objects['P_DeepIncluded'].location = (bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub2'].exclude, bpy.context.view_layer.layer_collection.children['Collection'].children['Sub'].children['Deep'].exclude)

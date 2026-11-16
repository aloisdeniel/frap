# SPDX-License-Identifier: GPL-3.0-or-later
"""Dump Blender's render camera matrices for the render-scene builder tests (WP 1.15, §19.2).

Runs INSIDE Blender 5.2.4:

    $BLENDER_BIN -b --factory-startup --python tools/blender/dump_camera_cases.py -- \
        --out crates/frap-render-scene/src/build/testdata/camera_cases.json

For every case the script sets up `scene.camera` (camera data, transform, render resolution,
percentage and pixel aspect) and records:

* `winmat`: `camera.calc_matrix_camera(depsgraph, x=winx, y=winy, scale_x=aspx,
  scale_y=aspy)` with `winx/winy = resolution * percentage // 100`
  (`BKE_render_resolution`). That RNA function runs `BKE_camera_params_init`,
  `_from_object`, `_compute_viewplane` and `_compute_matrix`, the same calls as the renderer's
  `RE_SetCamera` (OpenGL clip depth, column-major).
* `viewmat`: `matrix_world.normalized().inverted()`, as `RE_GetCameraModelMatrix`
  (`BKE_camera_multiview_model_matrix`, which applies `normalize_m4`) and `DRW_view_create`
  use it.
* `matrix_world` of the camera object (the test feeds it to the builder through a stub
  evaluator, so the comparison isolates the camera port).

Cases cover sensor fit (AUTO/HORIZONTAL/VERTICAL, landscape and portrait), lens shift, clip
range, orthographic scale, resolution percentage with odd sizes, non-square pixels, a scaled
and rotated camera object, a panoramic camera (treated as perspective by these functions) and
non-camera objects used as `scene.camera` (a spot light and an empty). Floats are written as
Python `repr` of the f32 value widened to double, which converts back to the same f32.
"""

import json
import math
import os
import sys

import bpy


def args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out = None
    if "--out" in argv:
        out = argv[argv.index("--out") + 1]
    return out


def cols(m):
    """4x4 mathutils Matrix -> 16 floats, column-major (glam `from_cols_array`)."""
    return [m[r][c] for c in range(4) for r in range(4)]


CAMERA_KEYS = (
    "type", "lens", "sensor_fit", "sensor_width", "sensor_height", "shift_x", "shift_y",
    "clip_start", "clip_end", "ortho_scale", "panorama_type",
)

CASES = [
    # name, object kind, camera data overrides, render overrides, object transform
    ("factory_default", "CAMERA", {}, {}, None),
    ("auto_portrait", "CAMERA", {}, {"resolution_x": 1080, "resolution_y": 1920}, None),
    ("auto_square", "CAMERA", {"lens": 35.0}, {"resolution_x": 512, "resolution_y": 512}, None),
    ("horizontal_portrait", "CAMERA", {"sensor_fit": "HORIZONTAL", "sensor_width": 36.0,
                                       "sensor_height": 24.0},
     {"resolution_x": 600, "resolution_y": 900}, None),
    ("horizontal_landscape", "CAMERA", {"sensor_fit": "HORIZONTAL", "sensor_width": 23.5},
     {"resolution_x": 800, "resolution_y": 450}, None),
    ("vertical_landscape", "CAMERA", {"sensor_fit": "VERTICAL", "sensor_height": 24.0},
     {"resolution_x": 1920, "resolution_y": 1080}, None),
    ("vertical_portrait", "CAMERA", {"sensor_fit": "VERTICAL", "sensor_height": 15.6,
                                     "lens": 85.0},
     {"resolution_x": 720, "resolution_y": 1280}, None),
    ("shift_landscape", "CAMERA", {"shift_x": 0.2, "shift_y": -0.1},
     {"resolution_x": 1280, "resolution_y": 720}, None),
    ("shift_portrait", "CAMERA", {"shift_x": -0.3, "shift_y": 0.25},
     {"resolution_x": 720, "resolution_y": 1280}, None),
    ("shift_vertical_fit", "CAMERA", {"sensor_fit": "VERTICAL", "shift_x": 0.15,
                                      "shift_y": 0.05},
     {"resolution_x": 1000, "resolution_y": 500}, None),
    ("clip_range", "CAMERA", {"clip_start": 0.01, "clip_end": 5000.0, "lens": 24.0}, {}, None),
    ("ortho_landscape", "CAMERA", {"type": "ORTHO", "ortho_scale": 7.3142858},
     {"resolution_x": 1920, "resolution_y": 1080}, None),
    ("ortho_portrait", "CAMERA", {"type": "ORTHO", "ortho_scale": 12.5, "clip_start": 0.5,
                                  "clip_end": 40.0},
     {"resolution_x": 600, "resolution_y": 1000}, None),
    ("ortho_shift_vertical", "CAMERA", {"type": "ORTHO", "ortho_scale": 3.0, "shift_x": 0.1,
                                        "shift_y": -0.2, "sensor_fit": "VERTICAL"},
     {"resolution_x": 640, "resolution_y": 480}, None),
    ("percentage_odd", "CAMERA", {"lens": 42.0},
     {"resolution_x": 1001, "resolution_y": 777, "resolution_percentage": 37}, None),
    ("pixel_aspect_wide", "CAMERA", {"shift_x": 0.1},
     {"resolution_x": 800, "resolution_y": 600, "pixel_aspect_x": 2.0, "pixel_aspect_y": 1.0},
     None),
    ("pixel_aspect_tall_auto_flip", "CAMERA", {},
     {"resolution_x": 800, "resolution_y": 600, "pixel_aspect_x": 1.0, "pixel_aspect_y": 1.5},
     None),
    ("pixel_aspect_ortho", "CAMERA", {"type": "ORTHO", "ortho_scale": 5.0},
     {"resolution_x": 400, "resolution_y": 400, "pixel_aspect_x": 1.0, "pixel_aspect_y": 2.0},
     None),
    ("scaled_rotated_object", "CAMERA", {"lens": 30.0}, {},
     {"location": (3.0, -7.5, 2.25), "rotation_euler": (1.2, 0.3, -2.4),
      "scale": (2.0, 0.5, 3.0)}),
    ("panoramic", "CAMERA", {"type": "PANO"}, {"resolution_x": 960, "resolution_y": 480}, None),
    ("spot_light_as_camera", "SPOT", {"spot_size": 1.2}, {"resolution_x": 640,
                                                         "resolution_y": 480},
     {"location": (1.0, 2.0, 3.0), "rotation_euler": (0.4, 0.0, 0.9), "scale": (1, 1, 1)}),
    ("empty_as_camera", "EMPTY", {}, {"resolution_x": 300, "resolution_y": 200},
     {"location": (-2.0, 0.5, 1.0), "rotation_euler": (1.5707964, 0.0, 0.0),
      "scale": (1, 1, 1)}),
]


def main():
    out = args()
    scene = bpy.context.scene
    base_render = {
        k: getattr(scene.render, k)
        for k in ("resolution_x", "resolution_y", "resolution_percentage", "pixel_aspect_x",
                  "pixel_aspect_y")
    }
    factory_cam = bpy.data.objects["Camera"]
    cases = []
    for name, kind, data_over, render_over, xform in CASES:
        for k, v in base_render.items():
            setattr(scene.render, k, v)
        for k, v in render_over.items():
            setattr(scene.render, k, v)
        if kind == "CAMERA":
            data = bpy.data.cameras.new("Cam_" + name)
            # Factory camera values (the factory Camera differs from a fresh one).
            data.clip_end = factory_cam.data.clip_end
            data.ortho_scale = factory_cam.data.ortho_scale
            for k, v in data_over.items():
                setattr(data, k, v)
            ob = bpy.data.objects.new("Ob_" + name, data)
        elif kind == "SPOT":
            data = bpy.data.lights.new("Spot_" + name, "SPOT")
            for k, v in data_over.items():
                setattr(data, k, v)
            ob = bpy.data.objects.new("Ob_" + name, data)
        else:
            ob = bpy.data.objects.new("Ob_" + name, None)
        scene.collection.objects.link(ob)
        if xform is None:
            ob.matrix_world = factory_cam.matrix_world.copy()
        else:
            ob.location = xform["location"]
            ob.rotation_euler = xform["rotation_euler"]
            ob.scale = xform["scale"]
        scene.camera = ob
        bpy.context.view_layer.update()
        depsgraph = bpy.context.evaluated_depsgraph_get()
        r = scene.render
        winx = r.resolution_x * r.resolution_percentage // 100
        winy = r.resolution_y * r.resolution_percentage // 100
        winmat = ob.calc_matrix_camera(depsgraph, x=winx, y=winy, scale_x=r.pixel_aspect_x,
                                       scale_y=r.pixel_aspect_y)
        viewmat = ob.matrix_world.normalized().inverted()
        case = {
            "name": name,
            "object_type": ob.type,
            "render": {
                "resolution_x": r.resolution_x,
                "resolution_y": r.resolution_y,
                "resolution_percentage": r.resolution_percentage,
                "pixel_aspect_x": r.pixel_aspect_x,
                "pixel_aspect_y": r.pixel_aspect_y,
            },
            "winx": winx,
            "winy": winy,
            "matrix_world": cols(ob.matrix_world),
            "winmat": cols(winmat),
            "viewmat": cols(viewmat),
        }
        if kind == "CAMERA":
            case["camera"] = {k: getattr(ob.data, k) for k in CAMERA_KEYS}
        elif kind == "SPOT":
            case["spot_size"] = ob.data.spot_size
        cases.append(case)
    text = json.dumps({"blender": bpy.app.version_string, "cases": cases}, indent=1)
    if out:
        os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
        with open(out, "w", encoding="utf-8") as f:
            f.write(text + "\n")
    else:
        print(text)


main()

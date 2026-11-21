// SPDX-License-Identifier: GPL-3.0-or-later
//! The colour-management check of a loaded file (WP 7.6): Blender runs
//! `IMB_colormanagement_check_file_config` on every file it reads, so a display, view
//! transform or look the OCIO config does not offer (on that display) is replaced, with a log
//! line. A `.frap` can hold such a view: an output override's view stays as it is when its own
//! display changes (`render.image_settings.view_settings.view_transform = 'Filmic'`, then
//! `display_settings.display_device = 'Display P3'`, which has no Filmic).
//!
//! Ported from blender/source/blender/imbuf/intern/colormanagement.cc
//! (`IMB_colormanagement_check_file_config`, `colormanage_check_display_settings`,
//! `colormanage_check_view_settings`, `colormanage_find_matching_view_name`; Blender 5.2.4,
//! GPL-2.0-or-later), for the scene and scene-output settings (frap has no sequencer, movie
//! clips or compositing node trees; image colour spaces are checked on image load). Verified on
//! 5.2.4: saving that override and opening the file logs `scene output view "Filmic" not
//! found, setting to "Standard".` and keeps the look `Medium High Contrast`; `AgX` on
//! `Rec.2100-PQ` becomes `AgX - HDR 1000 nits` (the first view whose name starts with the old
//! one) and keeps `AgX - Punchy`. frap reports each log line as a WARNING of the operator that
//! loads the file (Blender writes it to the console).

use frap_scene::Main;
use frap_scene::color_view::{look_items, view_items};
use frap_scene::generated::plain::{ColorManagedDisplaySettings, ColorManagedViewSettings};
use smol_str::SmolStr;

/// The displays of Blender 5.2.4's `config.ocio`, in config order; the first is the default.
const DISPLAYS: &[&str] = &[
    "sRGB",
    "Display P3",
    "Rec.1886",
    "Rec.2020",
    "Rec.2100-PQ",
    "Rec.2100-HLG",
];

/// The views of a display of the config (`None` for a display the config does not have).
fn display_views(display: &str) -> Option<&'static [&'static str]> {
    DISPLAYS
        .contains(&display)
        .then(|| view_items(Some(display)))
}

/// `colormanage_check_display_settings`: an empty display becomes the default one silently; a
/// display the config lacks becomes the first one whose name starts with it or that it starts
/// with (`sRGB - Display` → `sRGB`), else the default, with a warning.
fn check_display(d: &mut ColorManagedDisplaySettings, what: &str, warnings: &mut Vec<String>) {
    let name = d.display_device.clone();
    if name.is_empty() {
        d.display_device = SmolStr::new(DISPLAYS[0]);
        return;
    }
    if DISPLAYS.contains(&name.as_str()) {
        return;
    }
    let new = DISPLAYS
        .iter()
        .find(|x| x.starts_with(name.as_str()) || name.starts_with(**x))
        .copied()
        .unwrap_or(DISPLAYS[0]);
    warnings.push(format!(
        "Display \"{name}\" used by {what} not found, setting to \"{new}\"."
    ));
    d.display_device = SmolStr::new(new);
}

/// `colormanage_find_matching_view_name` on `views` (the display's, in config order; the first
/// is the display's default view).
fn matching_view(views: &[&'static str], view: &str) -> Option<&'static str> {
    // Untonemapped views of Blender's and ACES 2.0's naming.
    if view == "Standard" && views.contains(&"Un-tone-mapped") {
        return Some("Un-tone-mapped");
    }
    if view == "Un-tone-mapped" && views.contains(&"Standard") {
        return Some("Standard");
    }
    if let Some(v) = views
        .iter()
        .find(|v| v.starts_with(view) || view.starts_with(**v))
    {
        return Some(v);
    }
    if let Some((short, _)) = view.split_once(" - ")
        && let Some(v) = views.iter().find(|v| v.starts_with(short))
    {
        return Some(v);
    }
    views.first().copied()
}

/// `colormanage_check_view_settings`: the view must be one of the display's views (else the
/// matching one, with a warning), the look one of the config's looks (else `None`, with a
/// warning) that the view offers (else `None`, logged as INFO only), and `exposure` 0 with
/// `gamma` 0 becomes gamma 1.
fn check_view(
    d: &ColorManagedDisplaySettings,
    v: &mut ColorManagedViewSettings,
    what: &str,
    warnings: &mut Vec<String>,
) {
    let Some(views) = display_views(&d.display_device) else {
        return;
    };
    if v.view_transform.is_empty() {
        if let Some(first) = views.first() {
            v.view_transform = SmolStr::new(*first);
        }
    } else if !views.contains(&v.view_transform.as_str())
        && let Some(new) = matching_view(views, &v.view_transform)
    {
        // "Don't warn when only one view is available (e.g. fallback config)."
        if views.len() > 1 {
            warnings.push(format!(
                "{what} view \"{}\" not found, setting to \"{new}\".",
                v.view_transform
            ));
        }
        v.view_transform = SmolStr::new(new);
    }
    if v.look.is_empty() {
        v.look = SmolStr::new("None");
    } else if v.look != "None" {
        let known = DISPLAYS
            .iter()
            .flat_map(|d| view_items(Some(d)).iter())
            .any(|view| look_items(view).contains(&v.look.as_str()));
        if !known {
            warnings.push(format!(
                "{what} look \"{}\" not found, setting default \"None\".",
                v.look
            ));
            v.look = SmolStr::new("None");
        } else if !look_items(&v.view_transform).contains(&v.look.as_str()) {
            // `CLOG_INFO`: not shown at the default log level.
            v.look = SmolStr::new("None");
        }
    }
    if v.exposure == 0.0 && v.gamma == 0.0 {
        v.exposure = 0.0;
        v.gamma = 1.0;
    }
}

/// `IMB_colormanagement_check_file_config` for every scene of `main`: the scene's display and
/// view settings (`scene`), then its render output's (`scene output`). Returns the log lines.
pub(crate) fn check_file_config(main: &mut Main) -> Vec<String> {
    let mut warnings = Vec::new();
    let scenes: Vec<_> = main.scenes.iter().map(|(r, _)| r).collect();
    let mut untracked = frap_scene::ChangeLog::default();
    for r in scenes {
        let Some(s) = main.scenes.get_mut(r, &mut untracked) else {
            continue;
        };
        check_display(&mut s.display_settings, "scene", &mut warnings);
        check_view(
            &s.display_settings,
            &mut s.view_settings,
            "scene",
            &mut warnings,
        );
        let ims = &mut s.render.image_settings;
        check_display(&mut ims.display_settings, "scene output", &mut warnings);
        check_view(
            &ims.display_settings,
            &mut ims.view_settings,
            "scene output",
            &mut warnings,
        );
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(
        display: &str,
        view: &str,
        look: &str,
    ) -> (ColorManagedDisplaySettings, ColorManagedViewSettings) {
        let d = ColorManagedDisplaySettings {
            display_device: display.into(),
            ..Default::default()
        };
        let v = ColorManagedViewSettings {
            view_transform: view.into(),
            look: look.into(),
            ..Default::default()
        };
        (d, v)
    }

    /// The cases verified on 5.2.4.
    #[test]
    fn views_the_display_lacks_fall_back_as_blender() {
        let mut w = Vec::new();
        let (d, mut v) = settings("Display P3", "Filmic", "Medium High Contrast");
        check_view(&d, &mut v, "scene output", &mut w);
        assert_eq!(
            (v.view_transform.as_str(), v.look.as_str()),
            ("Standard", "Medium High Contrast")
        );
        assert_eq!(
            w,
            ["scene output view \"Filmic\" not found, setting to \"Standard\"."]
        );
        let mut w = Vec::new();
        let (d, mut v) = settings("Rec.2100-PQ", "AgX", "AgX - Punchy");
        check_view(&d, &mut v, "scene output", &mut w);
        assert_eq!(
            (v.view_transform.as_str(), v.look.as_str()),
            ("AgX - HDR 1000 nits", "AgX - Punchy")
        );
        assert_eq!(w.len(), 1);
        // A view the display offers is kept silently.
        let mut w = Vec::new();
        let (d, mut v) = settings("sRGB", "Filmic", "None");
        check_view(&d, &mut v, "scene", &mut w);
        assert_eq!(v.view_transform, "Filmic");
        assert!(w.is_empty());
    }

    #[test]
    fn matching_views_follow_the_name_rules() {
        let pq = view_items(Some("Rec.2100-PQ"));
        assert_eq!(
            matching_view(pq, "ACES 2.0"),
            Some("ACES 2.0 - HDR 500 nits")
        );
        assert_eq!(matching_view(pq, "Filmic Log"), Some("Standard"));
        let srgb = view_items(Some("sRGB"));
        assert_eq!(matching_view(srgb, "AgX - SDR"), Some("AgX"));
        assert_eq!(
            matching_view(srgb, "ACES 1.3 - HDR 1000 nits"),
            Some("ACES 1.3")
        );
    }

    #[test]
    fn unknown_displays_fall_back_to_a_prefix_or_srgb() {
        let mut w = Vec::new();
        let (mut d, _) = settings("sRGB - Display", "", "");
        check_display(&mut d, "scene", &mut w);
        assert_eq!(d.display_device, "sRGB");
        assert_eq!(
            w,
            ["Display \"sRGB - Display\" used by scene not found, setting to \"sRGB\"."]
        );
        let (mut d, _) = settings("Bogus", "", "");
        check_display(&mut d, "scene", &mut w);
        assert_eq!(d.display_device, "sRGB");
    }
}

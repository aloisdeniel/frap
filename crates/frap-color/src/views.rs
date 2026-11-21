// SPDX-License-Identifier: GPL-3.0-or-later
//! The view transforms each display offers in Blender 5.2.4's OCIO config
//! (`view_settings.view_transform` enum items, `rna_ColorManagedViewSettings_view_transform_itemf`
//! → `IMB_colormanagement_view_items_add`), and the view a display change keeps
//! (`IMB_colormanagement_validate_settings`).
//!
//! Verified on 5.2.4 (`blender -b --factory-startup`): every view name on every display, and
//! every (display, view) → display change; the HDR displays offer per-peak variants named
//! `"<View> - HDR <n> nits"` / `"<View> - SDR"`, and a display change keeps the view, else the
//! first view of the new display with the same base name (the part before `" - "`), else
//! `Standard` (`Filmic` → `Display P3` reads `Standard`, `ACES 2.0` → `Rec.2100-PQ` reads
//! `ACES 2.0 - HDR 500 nits`, `AgX - SDR` → `sRGB` reads `AgX`).

const SRGB: &[&str] = &[
    "Standard",
    "ACES 1.3",
    "ACES 2.0",
    "Khronos PBR Neutral",
    "AgX",
    "Filmic",
    "Filmic Log",
    "False Color",
    "Raw",
];

/// Display P3, Rec.1886 and Rec.2020.
const SDR_WIDE: &[&str] = &[
    "Standard",
    "ACES 1.3",
    "ACES 2.0",
    "AgX",
    "False Color",
    "Raw",
];

const REC2100_PQ: &[&str] = &[
    "Standard",
    "ACES 1.3 - HDR 1000 nits",
    "ACES 1.3 - HDR 2000 nits",
    "ACES 1.3 - HDR 4000 nits",
    "ACES 1.3 - SDR",
    "ACES 2.0 - HDR 500 nits",
    "ACES 2.0 - HDR 1000 nits",
    "ACES 2.0 - HDR 2000 nits",
    "ACES 2.0 - HDR 4000 nits",
    "ACES 2.0 - SDR",
    "AgX - HDR 1000 nits",
    "AgX - SDR",
    "False Color",
    "Raw",
];

const REC2100_HLG: &[&str] = &[
    "Standard",
    "ACES 1.3 - HDR 1000 nits",
    "ACES 1.3 - SDR",
    "ACES 2.0 - HDR 1000 nits",
    "ACES 2.0 - SDR",
    "AgX - HDR 1000 nits",
    "AgX - SDR",
    "False Color",
    "Raw",
];

/// The view transforms `display` offers, in config order (`None` for a display the config
/// does not have).
pub fn display_views(display: &str) -> Option<&'static [&'static str]> {
    Some(match display {
        "sRGB" => SRGB,
        "Display P3" | "Rec.1886" | "Rec.2020" => SDR_WIDE,
        "Rec.2100-PQ" => REC2100_PQ,
        "Rec.2100-HLG" => REC2100_HLG,
        _ => return None,
    })
}

/// Whether `display` offers `view` (`true` for a display the config does not have). A view a
/// display does not offer can stay stored on an owner Blender does not validate
/// (`render.image_settings` after its own display changed); Blender 5.2.4 reads it as `''` and
/// crashes rendering it (`create_ocio_display_processor`).
pub fn offered(display: &str, view: &str) -> bool {
    display_views(display).is_none_or(|views| views.contains(&view))
}

/// The view family of `view`: the part before `" - "` (`"AgX - HDR 1000 nits"` → `"AgX"`).
pub fn view_base(view: &str) -> &str {
    view.split_once(" - ").map_or(view, |(b, _)| b)
}

/// The view a change to `display` leaves in place of `view`: `view` when the display offers it,
/// else the first view of the display with the same base name, else `Standard`. `None` for a
/// display the config does not have (Blender keeps the settings).
pub fn validate_for_display(display: &str, view: &str) -> Option<&'static str> {
    let views = display_views(display)?;
    if let Some(v) = views.iter().find(|v| **v == view) {
        return Some(v);
    }
    let base = view_base(view);
    Some(
        views
            .iter()
            .find(|v| view_base(v) == base)
            .copied()
            .unwrap_or("Standard"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_changes_keep_the_view_family() {
        assert_eq!(
            validate_for_display("Display P3", "Filmic"),
            Some("Standard")
        );
        assert_eq!(
            validate_for_display("Rec.2020", "Khronos PBR Neutral"),
            Some("Standard")
        );
        assert_eq!(validate_for_display("Rec.1886", "AgX"), Some("AgX"));
        assert_eq!(
            validate_for_display("Rec.2100-PQ", "ACES 2.0"),
            Some("ACES 2.0 - HDR 500 nits")
        );
        assert_eq!(
            validate_for_display("Rec.2100-HLG", "ACES 2.0"),
            Some("ACES 2.0 - HDR 1000 nits")
        );
        assert_eq!(
            validate_for_display("Rec.2100-HLG", "ACES 1.3 - HDR 4000 nits"),
            Some("ACES 1.3 - HDR 1000 nits")
        );
        assert_eq!(validate_for_display("sRGB", "AgX - SDR"), Some("AgX"));
        assert_eq!(
            validate_for_display("Rec.2100-PQ", "Filmic"),
            Some("Standard")
        );
        assert_eq!(validate_for_display("Bogus", "Filmic"), None);
        assert!(!offered("Display P3", "Filmic"));
        assert!(offered("sRGB", "Filmic"));
        assert!(offered("Bogus", "Filmic"));
    }
}

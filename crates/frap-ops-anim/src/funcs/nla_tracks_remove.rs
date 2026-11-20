// SPDX-License-Identifier: GPL-3.0-or-later
//! `AnimData.nla_tracks.remove(track)` (`rna_NlaTrack_remove`,
//! blender/source/blender/makesrna/intern/rna_animation.cc, Blender 5.2.4): removes the track
//! with its strips. A track of another AnimData raises `RuntimeError: Error: NlaTrack '…'
//! cannot be removed`.

use frap_anim::rna::anim_data::AdtLoc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::nla_tracks_new::adt;

#[rna_func(srna = "NlaTracks", func = "remove")]
fn nla_tracks_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let Some(Value::Ptr(p)) = args.get("track") else {
        return Err(adt::removed("NlaTrack"));
    };
    let t = match adt::loc_of(h, p) {
        Some((AdtLoc::Track(t), None)) if p.root == this.root => t,
        _ => {
            let name = p
                .path
                .last()
                .map(|s| match s {
                    frap_rna::Seg::Key(k) => k.to_string(),
                    _ => String::new(),
                })
                .unwrap_or_default();
            return Err(adt::error(format!("NlaTrack '{name}' cannot be removed")));
        }
    };
    adt::with_adt(h, this, |a| {
        a.nla_tracks.remove(t);
        Ok(Value::None)
    })
}

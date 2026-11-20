// SPDX-License-Identifier: GPL-3.0-or-later
//! `AnimData.nla_tracks.new(prev=None) -> NlaTrack` (`rna_NlaTrack_new`,
//! blender/source/blender/makesrna/intern/rna_animation.cc, Blender 5.2.4): a new track
//! (`NlaTrack`, `NlaTrack.001`, …, selected) on top of the stack, or right above `prev`; it
//! becomes the active track.
//!
//! [`adt`] holds what the `NlaTracks` / `NlaStrips` functions share.

use frap_rna::{RnaError, RnaPtr, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use crate::keying::nla;

/// Locating the `AnimData` (and its tracks and strips) a function's pointers name.
pub(crate) mod adt {
    use frap_anim::AnimData;
    use frap_anim::rna::anim_data::{AdtLoc, locate};
    use frap_rna::{PyExc, RnaError, RnaErrorKind, RnaPtr, Seg};
    use frap_scene::Host;

    use crate::keying::{adt_slot_mut, report_error};

    /// `RuntimeError` for a pointer whose struct is gone.
    pub(crate) fn removed(what: &str) -> RnaError {
        RnaError::new(
            RnaErrorKind::Other,
            PyExc::RuntimeError,
            format!("StructRNA of type {what} has been removed"),
        )
    }

    /// The path of `p` below `animation_data`.
    pub(crate) fn below(p: &RnaPtr) -> Option<&[Seg]> {
        let i = p
            .path
            .iter()
            .position(|s| *s == Seg::Prop("animation_data"))?;
        Some(&p.path[i + 1..])
    }

    /// Where `p` points inside the AnimData of its root (`None`: elsewhere).
    pub(crate) fn loc_of(h: &dyn Host, p: &RnaPtr) -> Option<(AdtLoc, Option<String>)> {
        let adt = frap_eval::animate::anim_data(h.main(), p.root)?;
        let (loc, coll) = locate(adt, below(p)?)?;
        Some((loc, coll.map(str::to_owned)))
    }

    /// Runs `f` on the AnimData `this` belongs to, mutably.
    pub(crate) fn with_adt<R>(
        h: &mut dyn Host,
        this: &RnaPtr,
        f: impl FnOnce(&mut AnimData) -> Result<R, RnaError>,
    ) -> Result<R, RnaError> {
        let (main, log) = h.main_mut();
        let adt = adt_slot_mut(main, log, this.root)
            .and_then(Option::as_mut)
            .ok_or_else(|| removed("AnimData"))?;
        f(adt)
    }

    /// `BKE_report(RPT_ERROR, …)`
    pub(crate) fn error(msg: impl std::fmt::Display) -> RnaError {
        report_error(msg)
    }
}

#[rna_func(srna = "NlaTracks", func = "new")]
fn nla_tracks_new(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    let prev = match args.get("prev") {
        Some(Value::Ptr(p)) => match adt::loc_of(h, p) {
            Some((frap_anim::rna::anim_data::AdtLoc::Track(t), None)) if p.root == this.root => {
                Some(t)
            }
            _ => return Err(adt::removed("NlaTrack")),
        },
        _ => None,
    };
    let name = adt::with_adt(h, this, |adt| {
        let t = match prev {
            Some(p) => nla::track_new_after(adt, Some(p)),
            None => nla::track_new_tail(adt),
        };
        nla::track_set_active(adt, t);
        Ok(adt.nla_tracks[t].name.clone())
    })?;
    Ok(Value::Ptr(Box::new(
        this.child(Seg::Key(name), &frap_spec_rna::NLA_TRACK),
    )))
}

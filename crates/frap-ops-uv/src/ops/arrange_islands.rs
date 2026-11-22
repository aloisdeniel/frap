// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.arrange_islands(initial_position='BOUNDING_BOX', axis='Y', align='MIN',
//! order='LARGE_TO_SMALL', margin=0.05)`, tier T1 (`tests/golden/cases/uv/edit_m8/arrange*.py`).
//! Port of `uv_arrange_islands_exec` and `uvedit_uv_islands_arrange`
//! (`editors/uvedit/uvedit_ops.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. The UV islands of the selection (`BM_uv_element_map_create`
//! with seams) of every mesh in Edit Mode with a UV map and a selected vertex are sorted by
//! bounds area (`LARGE_TO_SMALL` / `SMALL_TO_LARGE`) or by their minimum along `axis`
//! (`Fixed`) and placed one after the other along `axis`, `margin` apart, aligned on the other
//! axis by their minimum, maximum or centre (`NONE`: kept). The line starts at the selection's
//! bounds (`BOUNDING_BOX`), the (1, 1) UV tile grid (`UV_GRID`), the unit tile
//! (`ACTIVE_UDIM`: no tiled image) or the 2D cursor of frap's virtual Image Editor
//! (`CURSOR`, headless-extended: `uv.snap_cursor` places it; Blender without an Image Editor
//! starts at the origin). Each mesh continues where the previous one ended. The sort is
//! libc++'s `std::stable_sort` ([`stable_sort`]): its comparator is not strict, so equal-area
//! islands come out in that algorithm's order. Always finishes.

pub(crate) mod stable_sort;

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::stitch::element_map::UvElementMap;
use super::unwrap::uvedit;
use super::weld::common::{self, TILE_GRID_SHAPE};
use crate::select::{Uv, UvSel, op};

/// `UVAlignIslandMode`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Align {
    Max,
    Min,
    Center,
    None,
}

/// `UVAlignIslandOrder`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Order {
    LargeToSmall,
    SmallToLarge,
    Fixed,
}

/// `UVAlignIslandBounds`.
#[derive(Copy, Clone, Debug)]
struct IslandBounds {
    min: Uv,
    max: Uv,
    index: usize,
}

impl IslandBounds {
    /// `Bounds::size` (`math::abs(max - min)`).
    fn size(&self) -> Uv {
        [
            (self.max[0] - self.min[0]).abs(),
            (self.max[1] - self.min[1]).abs(),
        ]
    }
    /// `Bounds::center` (`(min + max) * 0.5`).
    fn center(&self) -> Uv {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
        ]
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::ArrangeIslands) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let initial_position = p.initial_position.value.identifier();
    let axis = usize::from(p.axis.value.identifier() == "Y");
    let align = match p.align.value.identifier() {
        "MAX" => Align::Max,
        "CENTER" => Align::Center,
        "NONE" => Align::None,
        _ => Align::Min,
    };
    let order = match p.order.value.identifier() {
        "SMALL_TO_LARGE" => Order::SmallToLarge,
        "Fixed" => Order::Fixed,
        _ => Order::LargeToSmall,
    };
    let margin = p.margin.value;
    let other_axis = (axis + 1) % 2;
    let ts = op::ts(cx);
    let objects = common::objects(cx);

    let mut position: Uv = [0.0, 0.0];
    let mut bmin: Uv = [0.0, 0.0];
    let mut bmax: Uv = [1.0, 1.0];
    match initial_position {
        "BOUNDING_BOX" => {
            let (min, max) = common::init_minmax();
            bmin = min;
            bmax = max;
            common::foreach_uv_multi(cx, &objects, ts, true, |uv| {
                common::minmax(&mut bmin, &mut bmax, uv);
            });
        }
        // No tiled image in the virtual Image Editor: the unit tile.
        "ACTIVE_UDIM" => {}
        "UV_GRID" => {
            bmax = [TILE_GRID_SHAPE[0] as f32, TILE_GRID_SHAPE[1] as f32];
        }
        _ => position = common::cursor(cx),
    }
    if initial_position != "CURSOR" {
        let b = IslandBounds {
            min: bmin,
            max: bmax,
            index: 0,
        };
        position[other_axis] = match align {
            Align::Min => bmin[other_axis],
            Align::Center => b.center()[other_axis],
            _ => bmax[other_axis],
        };
        position[axis] = bmin[axis];
    }

    for (_, me) in objects {
        if common::no_vert_selected(cx, me) {
            continue;
        }
        common::edit(cx, me, ts, |s| {
            let changed = islands_arrange(s, axis, align, order, margin, &mut position);
            ((), changed)
        })?;
    }
    Ok(OpStatus::Finished)
}

/// `uvedit_uv_islands_arrange`.
fn islands_arrange(
    s: &mut UvSel,
    axis: usize,
    align: Align,
    order: Order,
    margin: f32,
    position: &mut Uv,
) -> bool {
    let Some(map) = UvElementMap::create(s, true, false, true, true) else {
        return false;
    };
    let other_axis = (axis + 1) % 2;
    let mut bounds: Vec<IslandBounds> = (0..map.total_islands)
        .map(|i| {
            let (mut min, mut max) = common::init_minmax();
            for e in map.island(i) {
                common::minmax(&mut min, &mut max, s.uv(map.storage[e].l));
            }
            IslandBounds { min, max, index: i }
        })
        .collect();
    stable_sort::stable_sort(&mut bounds, &|a: &IslandBounds, b: &IslandBounds| {
        if order == Order::Fixed {
            return a.min[axis] < b.min[axis];
        }
        let (sa, sb) = (a.size(), b.size());
        let area_a = sa[0] * sa[1];
        let area_b = sb[0] * sb[1];
        if order == Order::LargeToSmall {
            area_a >= area_b
        } else {
            area_a < area_b
        }
    });
    let mut changed = false;
    for b in &bounds {
        for e in map.island(b.index) {
            let luv = &mut s.uv[map.storage[e].l.slot()];
            match align {
                Align::Min => luv[other_axis] += position[other_axis] - b.min[other_axis],
                Align::Center => luv[other_axis] += position[other_axis] - b.center()[other_axis],
                Align::Max => luv[other_axis] += position[other_axis] - b.max[other_axis],
                Align::None => {}
            }
            luv[axis] += position[axis] - b.min[axis];
        }
        position[axis] += b.size()[axis] + margin;
        changed = true;
    }
    changed
}

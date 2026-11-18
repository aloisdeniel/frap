// SPDX-License-Identifier: GPL-3.0-or-later
//! Least Squares Conformal Maps (`p_chart_lscm_*` of `uv_parametrizer.cc`): the sparse
//! least-squares system over a chart's triangles, pinned by two vertices when the chart has
//! fewer than two pins (`p_chart_extrema_verts`), solved with `faer` (Blender uses Eigen; the
//! result agrees within 1e-4, a tolerance and not exactness, WP 3.3 acceptance). Stub until WP 3.3.

// SPDX-License-Identifier: GPL-3.0-or-later
//! `corner_split()`: splits vertices by (vertex, corner normal rounded to 4 dp, every UV, every
//! corner colour, tangent) for the glTF exporter and the renderer [GLTF §7.3]; the default cube
//! gives 24 vertices (ARCHITECTURE §6.1). WP 1.4.

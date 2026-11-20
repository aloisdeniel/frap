// SPDX-License-Identifier: GPL-3.0-or-later
//! RLight → GPU light records: EEVEE's power conversions per light type (point/spot `1/(4π²r²)`-style factors, sun irradiance, area normalisation; verified by the diffuse-plane golden), `use_temperature` blackbody, spot cone and blend, `diffuse/specular_factor`.
//! Owned by WP 5.1 (M5); a stub until it lands (M5+M6 W0).

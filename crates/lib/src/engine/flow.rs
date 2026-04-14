//! Irrigation flow: while a valve is open, compute the per-zone
//! inflow rate from the zone's emitter spec and area.
//!
//! Gallons-per-hour are converted to mm/hour over the zone's area so
//! the soil module sees a flux directly comparable to rain.  Where
//! the zone does not declare an explicit emitter count, the function
//! derives a plausible one from the emitter shape and the zone's
//! plant kind — see [`derive_emitter_count`].

use crate::catalog::{EmitterShape, EmitterSpec};
use crate::sim::zone::{PlantKind, Zone};

/// Convert zone-area square-feet to square-metres.  One square foot
/// is 0.092903 m²; kept as a named constant so the conversion is
/// visible at the call site.
const SQ_FT_TO_SQ_M: f64 = 0.092_903;

/// Convert gallons to litres.  1 mm of water over 1 m² is exactly
/// one litre, which is how the mm/hour flux comes out.
const GALLONS_TO_LITRES: f64 = 3.785_411_784;

/// Derive the number of emitters a zone carries when the fixture
/// does not declare one.  Picks a sensible default per emitter shape
/// and plant kind — inline drip at 12″ spacing scales with area;
/// point emitters scale with plant density.  These numbers are
/// rough; the catalog evolves to carry per-emitter densities in
/// v0.3 if that becomes worth the extra schema.
pub fn derive_emitter_count(zone: &Zone, emitter: &EmitterSpec) -> f64 {
  match emitter.shape {
    EmitterShape::InlineDrip => {
      let spacing_ft = emitter.inline_spacing_inches.unwrap_or(12.0) / 12.0;
      // Assume parallel runs at the same spacing; area / spacing²
      // is a reasonable coverage estimate.
      zone.area_sq_ft / (spacing_ft * spacing_ft).max(0.01)
    }
    EmitterShape::PointEmitter => {
      // One emitter per N sq ft, N chosen by plant kind.
      let sqft_per_emitter = match zone.plant_kind {
        PlantKind::VeggieBed => 4.0,
        PlantKind::Shrub => 10.0,
        PlantKind::Perennial => 5.0,
        PlantKind::Tree => 20.0,
      };
      (zone.area_sq_ft / sqft_per_emitter).max(1.0)
    }
    EmitterShape::MicroSpray => {
      // Sprays cover more area each; far fewer needed than point
      // emitters.
      (zone.area_sq_ft / 40.0).max(1.0)
    }
    EmitterShape::Bubbler => {
      // One bubbler per tree basin or equivalent perennial cluster.
      (zone.area_sq_ft / 50.0).max(1.0)
    }
  }
}

/// Per-zone inflow rate in mm/hour while the zone's valve is open.
/// Zero when the valve is closed (callers gate on valve state before
/// calling).  The return value is a flux directly comparable to rain
/// so the soil module can add the two.
pub fn zone_inflow_mm_per_hour(zone: &Zone, emitter: &EmitterSpec) -> f64 {
  let emitter_count = derive_emitter_count(zone, emitter);
  let zone_area_m2 = zone.area_sq_ft * SQ_FT_TO_SQ_M;
  if zone_area_m2 <= 0.0 {
    return 0.0;
  }
  // gph × litres_per_gallon / area_m² = mm/hour.
  (emitter_count * emitter.flow_gph * GALLONS_TO_LITRES) / zone_area_m2
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::catalog::EmitterShape;
  use crate::sim::id::{
    EmitterSpecId, ManifoldInstanceId, SoilTypeId, YardId, ZoneId,
  };

  fn zone_veggie(area: f64) -> Zone {
    Zone {
      id: ZoneId::new("z"),
      yard_id: YardId::new("y"),
      manifold_id: ManifoldInstanceId::new("m"),
      plant_kind: PlantKind::VeggieBed,
      emitter_spec_id: EmitterSpecId::new("e"),
      soil_type_id: SoilTypeId::new("s"),
      area_sq_ft: area,
      notes: None,
    }
  }

  fn inline_drip() -> EmitterSpec {
    EmitterSpec {
      id: EmitterSpecId::new("inline-drip-12in"),
      name: "Inline Drip".into(),
      manufacturer: "Example".into(),
      price_usd_per_100: 30.0,
      shape: EmitterShape::InlineDrip,
      flow_gph: 0.9,
      min_inlet_psi: 15.0,
      pressure_compensating: true,
      inline_spacing_inches: Some(12.0),
      notes: None,
    }
  }

  #[test]
  fn inline_drip_count_scales_with_area() {
    let small = derive_emitter_count(&zone_veggie(10.0), &inline_drip());
    let big = derive_emitter_count(&zone_veggie(100.0), &inline_drip());
    assert!(big > small);
  }

  #[test]
  fn zero_area_produces_zero_flux() {
    assert_eq!(zone_inflow_mm_per_hour(&zone_veggie(0.0), &inline_drip()), 0.0);
  }

  #[test]
  fn inline_drip_flux_is_plausible() {
    // 50 sq ft veggie bed at 12" spacing, 0.9 GPH inline drip.
    let flux = zone_inflow_mm_per_hour(&zone_veggie(50.0), &inline_drip());
    // Should be in the tens of mm/hour — substantial but not
    // outlandish for a short scheduled burst.
    assert!(flux > 5.0 && flux < 60.0, "flux {flux} outside plausible range");
  }

  #[test]
  fn higher_gph_produces_higher_flux() {
    let mut e = inline_drip();
    let low = zone_inflow_mm_per_hour(&zone_veggie(50.0), &e);
    e.flow_gph = 2.0;
    let high = zone_inflow_mm_per_hour(&zone_veggie(50.0), &e);
    assert!(high > low);
  }
}

use std::{collections::HashMap, marker::PhantomData};

use thiserror::Error;

use crate::{
  nav_data::NodeRef, path::PathIndex, pathfinding, Archipelago,
  CoordinateSystem, IslandId, NodeType,
};

/// A point on the navigation meshes.
pub struct SampledPoint<'archipelago, CS: CoordinateSystem> {
  /// The point on the navigation meshes.
  point: CS::Coordinate,
  /// The node that the point is on.
  node_ref: NodeRef,
  /// Marker to prevent this object from out-living a borrow to the
  /// archipelago.
  marker: PhantomData<&'archipelago ()>,
}

// Manual Clone impl for `SampledPoint` to avoid the Clone bound on CS.
impl<'archipelago, CS: CoordinateSystem> Clone
  for SampledPoint<'archipelago, CS>
{
  fn clone(&self) -> Self {
    Self {
      point: self.point.clone(),
      node_ref: self.node_ref,
      marker: self.marker,
    }
  }
}

impl<CS: CoordinateSystem> SampledPoint<'_, CS> {
  /// Gets the point on the navigation meshes.
  pub fn point(&self) -> CS::Coordinate {
    self.point.clone()
  }

  /// Gets the island the sampled point is on.
  pub fn island(&self) -> IslandId {
    self.node_ref.island_id
  }
}

/// An error while sampling a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum SamplePointError {
  #[error("The sample point is too far from any island.")]
  OutOfRange,
  #[error("The navigation data of the archipelago has been mutated since the last update.")]
  NavDataDirty,
}

/// Finds the nearest point on the navigation meshes to (and within
/// `distance_to_node` of) `point`.
pub(crate) fn sample_point<CS: CoordinateSystem>(
  archipelago: &Archipelago<CS>,
  point: CS::Coordinate,
  distance_to_node: f32,
) -> Result<SampledPoint<'_, CS>, SamplePointError> {
  if archipelago.nav_data.dirty {
    return Err(SamplePointError::NavDataDirty);
  }
  let Some((point, node_ref)) = archipelago
    .nav_data
    .sample_point(CS::to_landmass(&point), distance_to_node)
  else {
    return Err(SamplePointError::OutOfRange);
  };

  Ok(SampledPoint {
    point: CS::from_landmass(&point),
    node_ref,
    marker: PhantomData,
  })
}

/// An error from finding a path between two sampled points.
#[derive(Clone, Copy, Debug, PartialEq, Error)]
pub enum FindPathError {
  #[error("The node type {0:?} had a cost of {1}, which is non-positive.")]
  NonPositiveNodeTypeCost(NodeType, f32),
  #[error("No path was found between the start and end points.")]
  NoPathFound,
}

/// Finds a straight-line path across the navigation meshes from `start_point`
/// to `end_point`.
pub(crate) fn find_path<'a, CS: CoordinateSystem>(
  archipelago: &'a Archipelago<CS>,
  start_point: &SampledPoint<'a, CS>,
  end_point: &SampledPoint<'a, CS>,
  override_node_type_costs: &HashMap<NodeType, f32>,
) -> Result<Vec<CS::Coordinate>, FindPathError> {
  // This assert can actually be triggered. This can happen if a user samples
  // points from one archipelago, but finds a path in a **different**
  // archipelago. This seems almost malicious though, so I don't think we should
  // handle it at all. I'd rather the "wins" we get from avoiding
  // double-sampling (in cases where the user samples a point to check for
  // validity and then finds a path).
  assert!(!archipelago.nav_data.dirty, "The navigation data has been mutated, but we have SampledPoints, so this should be impossible.");

  for (node_type, cost) in override_node_type_costs.iter() {
    if *cost <= 0.0 {
      return Err(FindPathError::NonPositiveNodeTypeCost(*node_type, *cost));
    }
  }

  let Some(path) = pathfinding::find_path(
    &archipelago.nav_data,
    start_point.node_ref,
    end_point.node_ref,
    override_node_type_costs,
  )
  .path
  else {
    return Err(FindPathError::NoPathFound);
  };

  let mut current_index = PathIndex::from_corridor_index(0, 0);
  let mut current_point = CS::to_landmass(&start_point.point());

  let last_index = path.last_index();
  let last_point = CS::to_landmass(&end_point.point());

  let mut path_points = vec![start_point.point()];
  while current_index != last_index {
    (current_index, current_point) = path.find_next_point_in_straight_path(
      &archipelago.nav_data,
      current_index,
      current_point,
      last_index,
      last_point,
    );
    path_points.push(CS::from_landmass(&current_point));
  }

  Ok(path_points)
}

#[cfg(test)]
#[path = "query_test.rs"]
mod test;

use std::collections::HashMap;

use glam::{swizzles::Vec3Swizzles, Vec3};

use crate::BoundingBox;

/// A navigation mesh.
#[derive(Clone)]
pub struct NavigationMesh {
  /// The bounds of the mesh data itself. This should be a tight bounding box
  /// around the vertices of the navigation mesh. This may be None to
  /// automatically compute this from the vertices.
  pub mesh_bounds: Option<BoundingBox>,
  /// The vertices that make up the polygons. The Y component is considered up.
  pub vertices: Vec<Vec3>,
  /// The polygons of the mesh. Polygons are indices to the `vertices` that
  /// make up the polygon. Polygons must be convex, and oriented
  /// counterclockwise. Polygons are assumed to be not self-intersecting.
  pub polygons: Vec<Vec<usize>>,
}

/// An error when validating a navigation mesh.
#[derive(Debug)]
pub enum ValidationError {
  /// A polygon is concave (or has edges in clockwise order). Stores the index
  /// of the polygon.
  ConcavePolygon(usize),
  /// A polygon was not big enough (less than 3 vertices). Stores the index of
  /// the polygon.
  NotEnoughVerticesInPolygon(usize),
  /// A polygon indexed an invalid vertex. Stores the index of the polygon.
  InvalidVertexIndexInPolygon(usize),
  /// A polygon contains a degenerate edge (an edge using the same vertex for
  /// both endpoints). Stores the index of the polygon.
  DegenerateEdgeInPolygon(usize),
  /// An edge is used by more than two polygons. Stores the indices of the two
  /// vertices that make up the edge.
  DoublyConnectedEdge(usize, usize),
}

impl NavigationMesh {
  /// Ensures required invariants of the navigation mesh, and computes
  /// additional derived properties to produce and optimized and validated
  /// navigation mesh. Returns an error if the navigation mesh is invalid in
  /// some way.
  pub fn validate(mut self) -> Result<ValidNavigationMesh, ValidationError> {
    if self.mesh_bounds.is_none() {
      if self.vertices.is_empty() {
        self.mesh_bounds = Some(BoundingBox::Empty);
      }
      self.mesh_bounds =
        Some(self.vertices.iter().fold(BoundingBox::Empty, |acc, &vertex| {
          acc.expand_to_point(vertex)
        }));
    }

    enum ConnectivityState {
      Disconnected,
      Boundary {
        polygon: usize,
        edge: usize,
      },
      Connected {
        polygon_1: usize,
        edge_1: usize,
        polygon_2: usize,
        edge_2: usize,
      },
    }
    let mut connectivity_set = HashMap::new();

    for (polygon_index, polygon) in self.polygons.iter().enumerate() {
      if polygon.len() < 3 {
        return Err(ValidationError::NotEnoughVerticesInPolygon(polygon_index));
      }

      for vertex_index in polygon {
        if *vertex_index >= self.vertices.len() {
          return Err(ValidationError::InvalidVertexIndexInPolygon(
            polygon_index,
          ));
        }
      }

      for i in 0..polygon.len() {
        let left_vertex =
          polygon[if i == 0 { polygon.len() - 1 } else { i - 1 }];
        let center_vertex = polygon[i];
        let right_vertex =
          polygon[if i == polygon.len() - 1 { 0 } else { i + 1 }];

        // Check if the edge is degenerate.

        let edge = if center_vertex < right_vertex {
          (center_vertex, right_vertex)
        } else {
          (right_vertex, center_vertex)
        };
        if edge.0 == edge.1 {
          return Err(ValidationError::DegenerateEdgeInPolygon(polygon_index));
        }

        // Derive connectivity for the edge.

        let state = connectivity_set
          .entry(edge)
          .or_insert(ConnectivityState::Disconnected);
        match state {
          ConnectivityState::Disconnected => {
            *state =
              ConnectivityState::Boundary { polygon: polygon_index, edge: i };
          }
          &mut ConnectivityState::Boundary {
            polygon: polygon_1,
            edge: edge_1,
            ..
          } => {
            *state = ConnectivityState::Connected {
              polygon_1,
              edge_1,
              polygon_2: polygon_index,
              edge_2: i,
            };
          }
          ConnectivityState::Connected { .. } => {
            return Err(ValidationError::DoublyConnectedEdge(edge.0, edge.1));
          }
        }

        // Check if the vertex is concave.

        let left_vertex = self.vertices[left_vertex].xz();
        let center_vertex = self.vertices[center_vertex].xz();
        let right_vertex = self.vertices[right_vertex].xz();

        let left_edge = left_vertex - center_vertex;
        let right_edge = right_vertex - center_vertex;

        // If right_edge is to the left of the left_edge, then the polygon is
        // concave. This is the equation for a 2D cross product.
        if right_edge.x * left_edge.y - right_edge.y * left_edge.x < 0.0 {
          return Err(ValidationError::ConcavePolygon(polygon_index));
        }
      }
    }

    let mut polygons = self
      .polygons
      .drain(..)
      .map(|polygon_vertices| ValidPolygon {
        bounds: polygon_vertices
          .iter()
          .fold(BoundingBox::Empty, |bounds, vertex| {
            bounds.expand_to_point(self.vertices[*vertex])
          }),
        center: polygon_vertices
          .iter()
          .map(|i| self.vertices[*i])
          .sum::<Vec3>()
          / polygon_vertices.len() as f32,
        connectivity: vec![None; polygon_vertices.len()],
        vertices: polygon_vertices,
      })
      .collect::<Vec<_>>();

    let mut boundary_edges = Vec::new();
    for connectivity_state in connectivity_set.values() {
      match connectivity_state {
        ConnectivityState::Disconnected => panic!("Value is never stored"),
        &ConnectivityState::Boundary { polygon, edge } => {
          boundary_edges
            .push(MeshEdgeRef { edge_index: edge, polygon_index: polygon });
        }
        &ConnectivityState::Connected {
          polygon_1,
          edge_1,
          polygon_2,
          edge_2,
        } => {
          let edge = polygons[polygon_1].get_edge_indices(edge_1);
          let edge_center =
            (self.vertices[edge.0] + self.vertices[edge.1]) / 2.0;
          let cost = polygons[polygon_1].center.distance(edge_center)
            + polygons[polygon_2].center.distance(edge_center);
          polygons[polygon_1].connectivity[edge_1] =
            Some(Connectivity { polygon_index: polygon_2, cost });
          polygons[polygon_2].connectivity[edge_2] =
            Some(Connectivity { polygon_index: polygon_1, cost });
        }
      }
    }

    Ok(ValidNavigationMesh {
      mesh_bounds: self.mesh_bounds.unwrap(),
      polygons,
      vertices: self.vertices,
      boundary_edges,
    })
  }
}

/// A navigation mesh which has been validated and derived data has been
/// computed.
#[derive(Debug, Clone)]
pub struct ValidNavigationMesh {
  /// The bounds of the mesh data itself. This is a tight bounding box around
  /// the vertices of the navigation mesh.
  pub(crate) mesh_bounds: BoundingBox,
  /// The vertices that make up the polygons.
  pub(crate) vertices: Vec<Vec3>,
  /// The polygons of the mesh.
  pub(crate) polygons: Vec<ValidPolygon>,
  /// The boundary edges in the navigation mesh. Edges are stored as pairs of
  /// vertices in a counter-clockwise direction. That is, moving along an edge
  /// (e.0, e.1) from e.0 to e.1 will move counter-clockwise along the
  /// boundary. The order of edges is undefined.
  pub(crate) boundary_edges: Vec<MeshEdgeRef>,
}

/// A valid polygon. This means the polygon is convex and indexes the `vertices`
/// Vec of the corresponding ValidNavigationMesh.
#[derive(PartialEq, Debug, Clone)]
pub(crate) struct ValidPolygon {
  /// The vertices are indexes to the `vertices` Vec of the corresponding
  /// ValidNavigationMesh.
  pub(crate) vertices: Vec<usize>,
  /// The connectivity of each edge in the polygon. This is the same length as
  /// the number of edges (which is equivalent to `self.vertices.len()`).
  /// Entries that are `None` correspond to the boundary of the navigation
  /// mesh, while `Some` entries are connected to another node.
  pub(crate) connectivity: Vec<Option<Connectivity>>,
  /// The bounding box of `vertices`.
  pub(crate) bounds: BoundingBox,
  /// The center of the polygon.
  pub(crate) center: Vec3,
}

#[derive(PartialEq, Debug, Clone)]
pub struct Connectivity {
  /// The index of the polygon that this edge leads to.
  pub polygon_index: usize,
  /// The cost of travelling across this connection.
  pub cost: f32,
}

/// A reference to an edge on a navigation mesh.
#[derive(PartialEq, Eq, Debug, Clone, Hash, Default)]
pub struct MeshEdgeRef {
  /// The index of the polygon that this edge belongs to.
  pub polygon_index: usize,
  /// The index of the edge within the polygon.
  pub edge_index: usize,
}

impl ValidNavigationMesh {
  /// Returns the bounds of the navigation mesh.
  pub fn get_bounds(&self) -> BoundingBox {
    self.mesh_bounds
  }

  // Gets the points that make up the specified edge.
  pub fn get_edge_points(&self, edge_ref: MeshEdgeRef) -> (Vec3, Vec3) {
    let polygon = &self.polygons[edge_ref.polygon_index];
    let left_vertex_index = polygon.vertices[edge_ref.edge_index];

    let right_vertex_index =
      if edge_ref.edge_index == polygon.vertices.len() - 1 {
        0
      } else {
        edge_ref.edge_index + 1
      };
    let right_vertex_index = polygon.vertices[right_vertex_index];

    (self.vertices[left_vertex_index], self.vertices[right_vertex_index])
  }

  /// Finds the node nearest to (and within `distance_to_node` of) `point`.
  /// Returns the point on the nav mesh nearest to `point` and the index of the
  /// polygon.
  pub(crate) fn sample_point(
    &self,
    point: Vec3,
    distance_to_node: f32,
  ) -> Option<(Vec3, usize)> {
    let sample_box = BoundingBox::new_box(point, point)
      .expand_by_size(Vec3::ONE * distance_to_node);

    fn project_to_triangle(triangle: (Vec3, Vec3, Vec3), point: Vec3) -> Vec3 {
      let triangle_deltas = (
        triangle.1 - triangle.0,
        triangle.2 - triangle.1,
        triangle.0 - triangle.2,
      );
      let triangle_deltas_flat = (
        triangle_deltas.0.xz(),
        triangle_deltas.1.xz(),
        triangle_deltas.2.xz(),
      );

      if triangle_deltas_flat.0.perp_dot(point.xz() - triangle.0.xz()) < 0.0 {
        let s = triangle_deltas_flat.0.dot(point.xz() - triangle.0.xz())
          / triangle_deltas_flat.0.length_squared();
        return triangle_deltas.0 * s.clamp(0.0, 1.0) + triangle.0;
      }
      if triangle_deltas_flat.1.perp_dot(point.xz() - triangle.1.xz()) < 0.0 {
        let s = triangle_deltas_flat.1.dot(point.xz() - triangle.1.xz())
          / triangle_deltas_flat.1.length_squared();
        return triangle_deltas.1 * s.clamp(0.0, 1.0) + triangle.1;
      }
      if triangle_deltas_flat.2.perp_dot(point.xz() - triangle.2.xz()) < 0.0 {
        let s = triangle_deltas_flat.2.dot(point.xz() - triangle.2.xz())
          / triangle_deltas_flat.2.length_squared();
        return triangle_deltas.2 * s.clamp(0.0, 1.0) + triangle.2;
      }

      let normal = -triangle_deltas.0.cross(triangle_deltas.2).normalize();
      let height = normal.dot(point - triangle.0) / normal.y;
      Vec3::new(point.x, point.y - height, point.z)
    }

    let mut best_node = None;

    for (polygon_index, polygon) in self.polygons.iter().enumerate() {
      if !sample_box.intersects_bounds(&polygon.bounds) {
        continue;
      }
      for i in 2..polygon.vertices.len() {
        let triangle =
          (polygon.vertices[0], polygon.vertices[i - 1], polygon.vertices[i]);
        let triangle = (
          self.vertices[triangle.0],
          self.vertices[triangle.1],
          self.vertices[triangle.2],
        );
        let projected_point = project_to_triangle(triangle, point);

        let distance_to_triangle = point.distance_squared(projected_point);
        if distance_to_triangle < distance_to_node * distance_to_node {
          let replace = match best_node {
            None => true,
            Some((_, _, previous_best_distance))
              if distance_to_triangle < previous_best_distance =>
            {
              true
            }
            _ => false,
          };
          if replace {
            best_node =
              Some((polygon_index, projected_point, distance_to_triangle));
          }
        }
      }
    }

    best_node.map(|(polygon_index, projected_point, _)| {
      (projected_point, polygon_index)
    })
  }
}

impl ValidPolygon {
  /// Determines the vertices corresponding to `edge`.
  pub(crate) fn get_edge_indices(&self, edge: usize) -> (usize, usize) {
    (
      self.vertices[edge],
      self.vertices[if edge == self.vertices.len() - 1 { 0 } else { edge + 1 }],
    )
  }
}

#[cfg(test)]
#[path = "nav_mesh_test.rs"]
mod test;

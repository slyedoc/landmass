use std::collections::HashMap;

use glam::{swizzles::Vec3Swizzles, Vec3};

// A navigation mesh.
#[derive(Clone)]
pub struct NavigationMesh {
  // The bounds of the region that this mesh is responsible for. This should be
  // a superset of `mesh_bounds`. This differs from `mesh_bounds` as for a
  // tiled world, `region_bounds` may be the bounds of the tile, while
  // `mesh_bounds` would be the bounds of the actual vertices. This may be None
  // to specify that the `region_bounds` should be automatically computed as
  // the `mesh_bounds`.
  pub region_bounds: Option<(Vec3, Vec3)>,
  // The bounds of the mesh data itself. This should be a tight bounding box
  // around the vertices of the navigation mesh. This may be None to
  // automatically compute this from the vertices.
  pub mesh_bounds: Option<(Vec3, Vec3)>,
  // The vertices that make up the polygons. The Y component is considered up.
  pub vertices: Vec<Vec3>,
  // The polygons of the mesh. Polygons are indices to the `vertices` that make
  // up the polygon. Polygons must be convex, and oriented counterclockwise.
  // Polygons are assumed to be not self-intersecting.
  pub polygons: Vec<Vec<usize>>,
}

// An error when validating a navigation mesh.
#[derive(Debug)]
pub enum ValidationError {
  // A polygon is concave (or has edges in clockwise order). Stores the index
  // of the polygon.
  ConcavePolygon(usize),
  // A polygon was not big enough (less than 3 vertices). Stores the index of
  // the polygon.
  NotEnoughVerticesInPolygon(usize),
  // A polygon indexed an invalid vertex. Stores the index of the polygon.
  InvalidVertexIndexInPolygon(usize),
  // A polygon contains a degenerate edge (an edge using the same vertex for
  // both endpoints). Stores the index of the polygon.
  DegenerateEdgeInPolygon(usize),
  // An edge is used by more than two polygons. Stores the indices of the two
  // vertices that make up the edge.
  DoublyConnectedEdge(usize, usize),
}

impl NavigationMesh {
  pub(crate) fn validate(
    mut self,
  ) -> Result<ValidNavigationMesh, ValidationError> {
    if self.mesh_bounds.is_none() {
      if self.vertices.is_empty() {
        self.mesh_bounds = Some((Vec3::ZERO, Vec3::ZERO));
      }
      let first_vertex = *self.vertices.first().unwrap();
      self.mesh_bounds = Some(
        self
          .vertices
          .iter()
          .skip(1)
          .fold((first_vertex, first_vertex), |acc, &vertex| {
            (acc.0.min(vertex), acc.1.max(vertex))
          }),
      );
    }
    if self.region_bounds.is_none() {
      self.region_bounds = self.mesh_bounds;
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

    let mut connectivity = vec![Vec::new(); self.polygons.len()];
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
          connectivity[polygon_1].push(Connectivity {
            edge_index: edge_1,
            polygon_index: polygon_2,
          });
          connectivity[polygon_2].push(Connectivity {
            edge_index: edge_2,
            polygon_index: polygon_1,
          });
        }
      }
    }

    Ok(ValidNavigationMesh {
      region_bounds: self.region_bounds.unwrap(),
      mesh_bounds: self.mesh_bounds.unwrap(),
      vertices: self.vertices,
      polygons: self.polygons,
      connectivity,
      boundary_edges,
    })
  }
}

// A navigation mesh which has been validated and derived data has been
// computed.
#[derive(Debug)]
pub struct ValidNavigationMesh {
  // The bounds of the region that this mesh is responsible for. This is a
  // superset of `mesh_bounds`.
  pub region_bounds: (Vec3, Vec3),
  // The bounds of the mesh data itself. This is a tight bounding box around
  // the vertices of the navigation mesh.
  pub mesh_bounds: (Vec3, Vec3),
  // The vertices that make up the polygons.
  pub vertices: Vec<Vec3>,
  // The polygons of the mesh. Each polygon is convex and indexes `vertices`.
  pub polygons: Vec<Vec<usize>>,
  // The connectivity for each polygon. Each polygon has a Vec of the pairs
  // of (edge index, node index).
  pub connectivity: Vec<Vec<Connectivity>>,
  // The boundary edges in the navigation mesh. Edges are stored as pairs of
  // vertices in a counter-clockwise direction. That is, moving along an edge
  // (e.0, e.1) from e.0 to e.1 will move counter-clockwise along the boundary.
  // The order of edges is undefined.
  pub boundary_edges: Vec<MeshEdgeRef>,
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Connectivity {
  // The index of the edge within the polygon.
  pub edge_index: usize,
  // The index of the polygon that this edge leads to.
  pub polygon_index: usize,
}

// A reference to an edge on a navigation mesh.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct MeshEdgeRef {
  // The index of the polygon that this edge belongs to.
  pub polygon_index: usize,
  // The index of the edge within the polygon.
  pub edge_index: usize,
}

impl ValidNavigationMesh {
  // Gets the points that make up the specified edge.
  pub fn get_edge_points(&self, edge_ref: MeshEdgeRef) -> (Vec3, Vec3) {
    let polygon = &self.polygons[edge_ref.polygon_index];
    let left_vertex_index = polygon[edge_ref.edge_index];
    let right_vertex_index = polygon[if edge_ref.edge_index == polygon.len() - 1
    {
      0
    } else {
      edge_ref.edge_index + 1
    }];

    (self.vertices[left_vertex_index], self.vertices[right_vertex_index])
  }
}

#[cfg(test)]
mod tests {
  use glam::Vec3;

  use crate::nav_mesh::{Connectivity, MeshEdgeRef};

  use super::{NavigationMesh, ValidationError};

  #[test]
  fn validation_computes_bounds_if_none() {
    let mut source_mesh = NavigationMesh {
      mesh_bounds: None,
      region_bounds: None,
      vertices: vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(2.0, 0.0, 1.0),
        Vec3::new(0.5, 0.5, 3.0),
        Vec3::new(0.75, -0.25, 4.0),
        Vec3::new(0.25, 0.0, 4.0),
      ],
      polygons: vec![vec![0, 1, 2], vec![3, 4, 5]],
    };

    let valid_mesh =
      source_mesh.clone().validate().expect("Validation succeeds.");
    assert_eq!(
      valid_mesh.mesh_bounds,
      (Vec3::new(0.0, -0.25, 0.0), Vec3::new(2.0, 1.0, 4.0))
    );
    assert_eq!(valid_mesh.region_bounds, valid_mesh.mesh_bounds);

    let region_bounds =
      (Vec3::new(-10.0, -10.0, -10.0), Vec3::new(10.0, 10.0, 10.0));
    source_mesh.region_bounds = Some(region_bounds);

    let valid_mesh =
      source_mesh.clone().validate().expect("Validation succeeds.");
    assert_eq!(
      valid_mesh.mesh_bounds,
      (Vec3::new(0.0, -0.25, 0.0), Vec3::new(2.0, 1.0, 4.0))
    );
    assert_eq!(valid_mesh.region_bounds, region_bounds);

    let fake_mesh_bounds =
      (Vec3::new(-5.0, -5.0, -5.0), Vec3::new(5.0, 5.0, 5.0));
    source_mesh.mesh_bounds = Some(fake_mesh_bounds);

    let valid_mesh =
      source_mesh.clone().validate().expect("Validation succeeds.");
    assert_eq!(valid_mesh.mesh_bounds, fake_mesh_bounds);
    assert_eq!(valid_mesh.region_bounds, region_bounds);
  }

  #[test]
  fn polygons_and_vertices_copied() {
    let source_mesh = NavigationMesh {
      mesh_bounds: None,
      region_bounds: None,
      vertices: vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(2.0, 0.0, 1.0),
        Vec3::new(0.5, 0.5, 3.0),
        Vec3::new(0.75, -0.25, 4.0),
        Vec3::new(0.25, 0.0, 4.0),
      ],
      polygons: vec![vec![0, 1, 2], vec![3, 4, 5]],
    };

    let valid_mesh =
      source_mesh.clone().validate().expect("Validation succeeds.");
    assert_eq!(valid_mesh.vertices, source_mesh.vertices);
    assert_eq!(valid_mesh.polygons, source_mesh.polygons);
  }

  #[test]
  fn error_on_concave_polygon() {
    let source_mesh = NavigationMesh {
      mesh_bounds: None,
      region_bounds: None,
      vertices: vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(1.0, 0.0, 0.0),
      ],
      polygons: vec![vec![0, 1, 2]],
    };

    let error = source_mesh
      .clone()
      .validate()
      .expect_err("Concave polygon should be detected.");
    match error {
      ValidationError::ConcavePolygon(polygon) => assert_eq!(polygon, 0),
      _ => panic!(
        "Wrong error variant! Expected ConcavePolygon but got: {:?}",
        error
      ),
    };
  }

  #[test]
  fn error_on_small_polygon() {
    let source_mesh = NavigationMesh {
      mesh_bounds: None,
      region_bounds: None,
      vertices: vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 1.0)],
      polygons: vec![vec![0, 1]],
    };

    let error = source_mesh
      .clone()
      .validate()
      .expect_err("Small polygon should be detected.");
    match error {
      ValidationError::NotEnoughVerticesInPolygon(polygon) => assert_eq!(polygon, 0),
      _ => panic!(
        "Wrong error variant! Expected NotEnoughVerticesInPolygon but got: {:?}",
        error
      ),
    };
  }

  #[test]
  fn error_on_bad_polygon_index() {
    let source_mesh = NavigationMesh {
      mesh_bounds: None,
      region_bounds: None,
      vertices: vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 1.0),
      ],
      polygons: vec![vec![0, 1, 3]],
    };

    let error = source_mesh
      .clone()
      .validate()
      .expect_err("Bad polygon index should be detected.");
    match error {
      ValidationError::InvalidVertexIndexInPolygon(polygon) => assert_eq!(polygon, 0),
      _ => panic!(
        "Wrong error variant! Expected InvalidVertexIndexInPolygon but got: {:?}",
        error
      ),
    };
  }

  #[test]
  fn error_on_degenerate_edge() {
    let source_mesh = NavigationMesh {
      mesh_bounds: None,
      region_bounds: None,
      vertices: vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 1.0),
      ],
      polygons: vec![vec![0, 1, 1, 2]],
    };

    let error = source_mesh
      .clone()
      .validate()
      .expect_err("Degenerate edge should be detected.");
    match error {
      ValidationError::DegenerateEdgeInPolygon(polygon) => {
        assert_eq!(polygon, 0)
      }
      _ => panic!(
        "Wrong error variant! Expected DegenerateEdgeInPolygon but got: {:?}",
        error
      ),
    };
  }

  #[test]
  fn error_on_doubly_connected_edge() {
    let source_mesh = NavigationMesh {
      mesh_bounds: None,
      region_bounds: None,
      vertices: vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(2.0, 0.0, 1.0),
        Vec3::new(2.0, 1.0, 0.0),
        Vec3::new(2.0, 1.0, 1.0),
      ],
      polygons: vec![vec![0, 1, 2], vec![1, 3, 4, 2], vec![1, 5, 6, 2]],
    };

    let error = source_mesh
      .clone()
      .validate()
      .expect_err("Doubly connected edge should be detected.");
    match error {
      ValidationError::DoublyConnectedEdge(vertex_1, vertex_2) => {
        assert_eq!((vertex_1, vertex_2), (1, 2));
      }
      _ => panic!(
        "Wrong error variant! Expected DoublyConnectedEdge but got: {:?}",
        error
      ),
    };
  }

  #[test]
  fn derives_connectivity_and_boundary_edges() {
    let source_mesh = NavigationMesh {
      mesh_bounds: None,
      region_bounds: None,
      vertices: vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(2.0, 0.0, 1.0),
        Vec3::new(3.0, 1.0, 0.0),
        Vec3::new(3.0, 1.0, 1.0),
        Vec3::new(1.0, 1.0, 2.0),
        Vec3::new(2.0, 1.0, 2.0),
      ],
      polygons: vec![
        vec![0, 1, 2],
        vec![1, 3, 4, 2],
        vec![3, 5, 6, 4],
        vec![2, 4, 8, 7],
      ],
    };

    let mut valid_mesh =
      source_mesh.clone().validate().expect("Validation succeeds.");

    // Sort connectivity and boundary edges to ensure the order is consistent
    // when comparing.
    for connectivity in valid_mesh.connectivity.iter_mut() {
      connectivity.sort_by_key(|connectivity| {
        connectivity.polygon_index * 100 + connectivity.edge_index
      });
    }
    valid_mesh.boundary_edges.sort_by_key(|boundary_edge| {
      boundary_edge.polygon_index * 100 + boundary_edge.edge_index
    });

    let expected_connectivity: [&[_]; 4] = [
      &[Connectivity { edge_index: 1, polygon_index: 1 }],
      &[
        Connectivity { edge_index: 3, polygon_index: 0 },
        Connectivity { edge_index: 1, polygon_index: 2 },
        Connectivity { edge_index: 2, polygon_index: 3 },
      ],
      &[Connectivity { edge_index: 3, polygon_index: 1 }],
      &[Connectivity { edge_index: 0, polygon_index: 1 }],
    ];
    assert_eq!(valid_mesh.connectivity, expected_connectivity);
    assert_eq!(
      valid_mesh.boundary_edges,
      [
        MeshEdgeRef { polygon_index: 0, edge_index: 0 },
        MeshEdgeRef { polygon_index: 0, edge_index: 2 },
        MeshEdgeRef { polygon_index: 1, edge_index: 0 },
        MeshEdgeRef { polygon_index: 2, edge_index: 0 },
        MeshEdgeRef { polygon_index: 2, edge_index: 1 },
        MeshEdgeRef { polygon_index: 2, edge_index: 2 },
        MeshEdgeRef { polygon_index: 3, edge_index: 1 },
        MeshEdgeRef { polygon_index: 3, edge_index: 2 },
        MeshEdgeRef { polygon_index: 3, edge_index: 3 },
      ]
    );
  }
}

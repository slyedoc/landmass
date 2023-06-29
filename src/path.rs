use glam::{Vec3, Vec3Swizzles};

use crate::{nav_data::NodeRef, NavigationData};

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Path {
  pub(crate) corridor: Vec<NodeRef>,
  pub(crate) portal_edge_index: Vec<usize>,
}

impl Path {
  fn get_portal_endpoints(
    &self,
    portal_index: usize,
    nav_data: &NavigationData,
  ) -> (Vec3, Vec3) {
    let node_ref = self.corridor[portal_index].clone();
    let edge = self.portal_edge_index[portal_index];

    let island_data = nav_data
      .islands
      .get(&node_ref.island_id)
      .expect("only called if path is still valid")
      .nav_data
      .as_ref()
      .expect("only called if path is still valid");
    let (left_vertex, right_vertex) = island_data.nav_mesh.polygons
      [node_ref.polygon_index]
      .get_edge_indices(edge);

    (
      island_data.transform.apply(island_data.nav_mesh.vertices[left_vertex]),
      island_data.transform.apply(island_data.nav_mesh.vertices[right_vertex]),
    )
  }

  pub(crate) fn find_next_point_in_straight_path(
    &self,
    nav_data: &NavigationData,
    start_index: usize,
    start_point: Vec3,
    end_index: usize,
    end_point: Vec3,
  ) -> (usize, Vec3) {
    let apex = start_point;
    let (mut left_index, mut right_index) = (start_index, start_index);

    let (mut current_left, mut current_right) = if start_index == end_index {
      (end_point, end_point)
    } else {
      self.get_portal_endpoints(start_index, nav_data)
    };

    fn triangle_area_2(point_0: Vec3, point_1: Vec3, point_2: Vec3) -> f32 {
      return (point_1.xz() - point_0.xz())
        .perp_dot(point_2.xz() - point_0.xz());
    }

    for portal_index in (start_index + 1)..=end_index {
      let (portal_left, portal_right) = if portal_index == end_index {
        (end_point, end_point)
      } else {
        self.get_portal_endpoints(portal_index, nav_data)
      };

      if triangle_area_2(apex, current_right, portal_right) <= 0.0 {
        if triangle_area_2(apex, current_left, portal_right) >= 0.0 {
          right_index = portal_index;
          current_right = portal_right;
        } else {
          return (left_index, current_left);
        }
      }

      if triangle_area_2(apex, current_left, portal_left) >= 0.0 {
        if triangle_area_2(apex, current_right, portal_left) <= 0.0 {
          left_index = portal_index;
          current_left = portal_left;
        } else {
          return (right_index, current_right);
        }
      }
    }

    (end_index, end_point)
  }
}

#[cfg(test)]
mod tests {
  use glam::Vec3;

  use crate::{
    nav_data::{NavigationData, NodeRef},
    nav_mesh::NavigationMesh,
    Archipelago, Transform,
  };

  use super::Path;

  fn collect_straight_path(
    path: &Path,
    nav_data: &NavigationData,
    start: (usize, Vec3),
    end: (usize, Vec3),
    iteration_limit: u32,
  ) -> Vec<(usize, Vec3)> {
    let mut straight_path = Vec::with_capacity(iteration_limit as usize);

    let mut current = start;
    let mut iterations = 0;
    while current.0 != end.0 && iterations < iteration_limit {
      iterations += 1;
      current = path.find_next_point_in_straight_path(
        nav_data, current.0, current.1, end.0, end.1,
      );
      straight_path.push(current);
    }

    straight_path
  }

  #[test]
  fn finds_next_point_for_organic_map() {
    let nav_mesh = NavigationMesh {
      mesh_bounds: None,
      vertices: vec![
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(4.0, 0.0, 1.0),
        Vec3::new(4.0, 0.0, 2.0),
        Vec3::new(2.0, 0.0, 3.0),
        Vec3::new(1.0, 0.0, 3.0),
        Vec3::new(0.0, 0.0, 2.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 0.0, 5.0),
        Vec3::new(2.0, 0.0, 5.0),
        Vec3::new(2.0, 0.0, 4.0),
        Vec3::new(3.0, 1.0, 5.0),
        Vec3::new(3.0, 1.0, 4.0),
        Vec3::new(3.0, -2.0, 4.0),
        Vec3::new(3.0, -2.0, 3.0),
      ],
      polygons: vec![
        vec![0, 1, 2, 3, 4, 5, 6, 7],
        vec![5, 4, 10, 9, 8],
        vec![9, 10, 12, 11],
        vec![10, 4, 14, 13],
      ],
    }
    .validate()
    .expect("Mesh is valid.");

    let mut archipelago = Archipelago::new();
    let island_id = archipelago.add_island(nav_mesh.mesh_bounds);
    archipelago.get_island_mut(island_id).set_nav_mesh(
      Transform { translation: Vec3::ZERO, rotation: 0.0 },
      nav_mesh,
      /* linkable_distance_to_region_edge= */ 0.01,
    );

    let path = Path {
      corridor: vec![
        NodeRef { island_id, polygon_index: 0 },
        NodeRef { island_id, polygon_index: 1 },
        NodeRef { island_id, polygon_index: 2 },
      ],
      portal_edge_index: vec![4, 2],
    };

    assert_eq!(
      collect_straight_path(
        &path,
        &archipelago.nav_data,
        /* start= */ (0, Vec3::new(3.0, 0.0, 1.5)),
        /* end= */ (2, Vec3::new(2.5, 0.5, 4.5)),
        /* iteration_limit= */ 3,
      ),
      [
        (0, Vec3::new(2.0, 0.0, 3.0)),
        (1, Vec3::new(2.0, 0.0, 4.0)),
        (2, Vec3::new(2.5, 0.5, 4.5)),
      ]
    );
  }

  #[test]
  fn finds_next_point_in_zig_zag() {
    let nav_mesh = NavigationMesh {
      mesh_bounds: None,
      vertices: vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 0.0, 2.0),
        Vec3::new(0.0, 0.0, 2.0),
        Vec3::new(1.0, 0.0, 3.0),
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::new(1.0, 0.0, 4.0),
        Vec3::new(0.0, 0.0, 4.0),
        Vec3::new(1.0, 0.0, 5.0), // Turn right
        Vec3::new(2.0, 0.0, 4.0),
        Vec3::new(2.0, 0.0, 5.0),
        Vec3::new(3.0, 0.0, 4.0),
        Vec3::new(3.0, 0.0, 5.0),
        Vec3::new(4.0, 0.0, 4.0),
        Vec3::new(4.0, 0.0, 5.0),
        Vec3::new(5.0, 0.0, 5.0), // Turn left
        Vec3::new(5.0, 0.0, 6.0),
        Vec3::new(4.0, 0.0, 6.0),
        Vec3::new(5.0, 0.0, 7.0),
        Vec3::new(4.0, 0.0, 7.0),
        Vec3::new(4.0, 0.0, 8.0), // Turn left
        Vec3::new(-3.0, 0.0, 8.0),
        Vec3::new(-3.0, 0.0, 7.0),
        Vec3::new(-4.0, 0.0, 8.0), // Turn right
        Vec3::new(-3.0, 0.0, 15.0),
        Vec3::new(-4.0, 0.0, 15.0),
      ],
      polygons: vec![
        vec![0, 1, 2, 3],
        vec![3, 2, 4, 5],
        vec![5, 4, 6, 7],
        vec![7, 6, 8, 9],
        vec![9, 8, 10],
        vec![10, 8, 11, 12],
        vec![12, 11, 13, 14],
        vec![14, 13, 15, 16],
        vec![16, 15, 17],
        vec![16, 17, 18, 19],
        vec![19, 18, 20, 21],
        vec![21, 20, 22],
        vec![21, 22, 23, 24],
        vec![24, 23, 25],
        vec![25, 23, 26, 27],
      ],
    }
    .validate()
    .expect("Mesh is valid.");

    let mut archipelago = Archipelago::new();
    let island_id = archipelago.add_island(nav_mesh.mesh_bounds);
    archipelago.get_island_mut(island_id).set_nav_mesh(
      Transform { translation: Vec3::ZERO, rotation: 0.0 },
      nav_mesh,
      /* linkable_distance_to_region_edge= */ 0.01,
    );

    let path = Path {
      corridor: vec![
        NodeRef { island_id, polygon_index: 0 },
        NodeRef { island_id, polygon_index: 1 },
        NodeRef { island_id, polygon_index: 2 },
        NodeRef { island_id, polygon_index: 3 },
        NodeRef { island_id, polygon_index: 4 },
        NodeRef { island_id, polygon_index: 5 },
        NodeRef { island_id, polygon_index: 6 },
        NodeRef { island_id, polygon_index: 7 },
        NodeRef { island_id, polygon_index: 8 },
        NodeRef { island_id, polygon_index: 9 },
        NodeRef { island_id, polygon_index: 10 },
        NodeRef { island_id, polygon_index: 11 },
        NodeRef { island_id, polygon_index: 12 },
        NodeRef { island_id, polygon_index: 13 },
        NodeRef { island_id, polygon_index: 14 },
      ],
      portal_edge_index: vec![2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 2, 2, 2, 1],
    };

    assert_eq!(
      collect_straight_path(
        &path,
        &archipelago.nav_data,
        /* start= */ (0, Vec3::new(0.5, 0.0, 0.5)),
        /* end= */ (14, Vec3::new(-3.5, 0.0, 14.0)),
        /* iteration_limit= */ 5,
      ),
      [
        (4, Vec3::new(1.0, 0.0, 4.0)),
        (8, Vec3::new(4.0, 0.0, 5.0)),
        (11, Vec3::new(4.0, 0.0, 7.0)),
        (13, Vec3::new(-3.0, 0.0, 8.0)),
        (14, Vec3::new(-3.5, 0.0, 14.0)),
      ]
    );
  }

  #[test]
  fn starts_at_end_index_goes_to_end_point() {
    let nav_mesh = NavigationMesh {
      mesh_bounds: None,
      vertices: vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 0.0, 2.0),
        Vec3::new(0.0, 0.0, 2.0),
      ],
      polygons: vec![vec![0, 1, 2, 3], vec![3, 2, 4, 5]],
    }
    .validate()
    .expect("Mesh is valid.");

    let mut archipelago = Archipelago::new();
    let island_id = archipelago.add_island(nav_mesh.mesh_bounds);
    archipelago.get_island_mut(island_id).set_nav_mesh(
      Transform { translation: Vec3::ZERO, rotation: 0.0 },
      nav_mesh,
      /* linkable_distance_to_region_edge= */ 0.01,
    );

    let path = Path {
      corridor: vec![
        NodeRef { island_id, polygon_index: 0 },
        NodeRef { island_id, polygon_index: 1 },
      ],
      portal_edge_index: vec![2],
    };

    assert_eq!(
      path.find_next_point_in_straight_path(
        &archipelago.nav_data,
        /* start_index= */ 1,
        /* start_point= */ Vec3::new(0.25, 0.0, 1.1),
        /* end_index= */ 1,
        /* end_point= */ Vec3::new(0.75, 0.0, 1.9),
      ),
      (1, Vec3::new(0.75, 0.0, 1.9))
    );
  }
}

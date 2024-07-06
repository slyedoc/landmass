use std::{cmp::Ordering, sync::Arc};

use glam::Vec3;

use crate::{
  coords::XYZ,
  debug::{DebugDrawer, LineType, PointType, TriangleType},
  Agent, Archipelago, NavigationMesh, Transform,
};

use super::draw_archipelago_debug;

struct FakeDrawer {
  points: Vec<(PointType, Vec3)>,
  lines: Vec<(LineType, [Vec3; 2])>,
  triangles: Vec<(TriangleType, [Vec3; 3])>,
}
impl DebugDrawer for FakeDrawer {
  fn add_point(&mut self, point_type: crate::debug::PointType, point: Vec3) {
    self.points.push((point_type, point));
  }

  fn add_line(&mut self, line_type: crate::debug::LineType, line: [Vec3; 2]) {
    self.lines.push((line_type, line));
  }

  fn add_triangle(
    &mut self,
    triangle_type: crate::debug::TriangleType,
    triangle: [Vec3; 3],
  ) {
    self.triangles.push((triangle_type, triangle));
  }
}

impl FakeDrawer {
  fn new() -> Self {
    Self { points: vec![], lines: vec![], triangles: vec![] }
  }

  fn sort(&mut self) {
    fn lex_order_points(a: Vec3, b: Vec3) -> Ordering {
      a.x
        .partial_cmp(&b.x)
        .unwrap()
        .then(a.y.partial_cmp(&b.y).unwrap())
        .then(a.z.partial_cmp(&b.z).unwrap())
    }
    self.points.sort_by(|a, b| a.0.cmp(&b.0).then(lex_order_points(a.1, b.1)));
    self.lines.sort_by(|a, b| {
      a.0
        .cmp(&b.0)
        .then(lex_order_points(a.1[0], b.1[0]))
        .then(lex_order_points(a.1[1], b.1[1]))
    });
    self.triangles.sort_by(|a, b| {
      a.0
        .cmp(&b.0)
        .then(lex_order_points(a.1[0], b.1[0]))
        .then(lex_order_points(a.1[1], b.1[1]))
        .then(lex_order_points(a.1[2], b.1[2]))
    });
  }
}

#[test]
fn draws_island_meshes_and_agents() {
  let nav_mesh = NavigationMesh::<XYZ> {
    vertices: vec![
      Vec3::new(1.0, 0.0, 0.0),
      Vec3::new(2.0, 0.0, 0.0),
      Vec3::new(4.0, 1.0, 0.0),
      Vec3::new(4.0, 2.0, 0.0),
      Vec3::new(2.0, 3.0, 0.0),
      Vec3::new(1.0, 3.0, 0.0),
      Vec3::new(0.0, 2.0, 0.0),
      Vec3::new(0.0, 1.0, 0.0),
      Vec3::new(1.0, 5.0, 0.0),
      Vec3::new(2.0, 5.0, 0.0),
      Vec3::new(2.0, 4.0, 0.0),
      Vec3::new(3.0, 5.0, 1.0),
      Vec3::new(3.0, 4.0, 1.0),
      Vec3::new(3.0, 4.0, -2.0),
      Vec3::new(3.0, 3.0, -2.0),
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
  let island_id = archipelago.add_island();
  const TRANSLATION: Vec3 = Vec3::ONE;
  archipelago.get_island_mut(island_id).set_nav_mesh(
    Transform { translation: TRANSLATION, rotation: 0.0 },
    Arc::new(nav_mesh),
  );

  let agent_id = archipelago.add_agent(Agent::create(
    /* position= */ Vec3::new(3.9, 1.5, 0.0) + TRANSLATION,
    /* velocity= */ Vec3::ZERO,
    /* radius= */ 1.0,
    /* max_velocity= */ 1.0,
  ));
  archipelago.get_agent_mut(agent_id).current_target =
    Some(Vec3::new(1.5, 4.5, 0.0) + TRANSLATION);

  // Update so everything is in sync.
  archipelago.update(1.0);

  let mut fake_drawer = FakeDrawer::new();
  draw_archipelago_debug(&archipelago, &mut fake_drawer);

  fake_drawer.sort();

  assert_eq!(
    fake_drawer.points,
    [
      (PointType::AgentPosition(agent_id), Vec3::new(3.9, 1.5, 0.0)),
      (PointType::TargetPosition(agent_id), Vec3::new(1.5, 4.5, 0.0)),
      (PointType::Waypoint(agent_id), Vec3::new(2.0, 3.0, 0.0)),
    ]
    .iter()
    .copied()
    .map(|(t, p)| (t, p + TRANSLATION))
    .collect::<Vec<_>>()
  );
  assert_eq!(
    fake_drawer.lines,
    [
      (
        LineType::BoundaryEdge,
        [Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 1.0, 0.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(1.0, 3.0, 0.0), Vec3::new(0.0, 2.0, 0.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(1.0, 5.0, 0.0), Vec3::new(1.0, 3.0, 0.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(2.0, 0.0, 0.0), Vec3::new(4.0, 1.0, 0.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(2.0, 3.0, 0.0), Vec3::new(3.0, 3.0, -2.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(2.0, 4.0, 0.0), Vec3::new(3.0, 4.0, 1.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(2.0, 5.0, 0.0), Vec3::new(1.0, 5.0, 0.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(3.0, 3.0, -2.0), Vec3::new(3.0, 4.0, -2.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(3.0, 4.0, -2.0), Vec3::new(2.0, 4.0, 0.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(3.0, 4.0, 1.0), Vec3::new(3.0, 5.0, 1.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(3.0, 5.0, 1.0), Vec3::new(2.0, 5.0, 0.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(4.0, 1.0, 0.0), Vec3::new(4.0, 2.0, 0.0)]
      ),
      (
        LineType::BoundaryEdge,
        [Vec3::new(4.0, 2.0, 0.0), Vec3::new(2.0, 3.0, 0.0)]
      ),
      //
      (
        LineType::ConnectivityEdge,
        [Vec3::new(2.0, 3.0, 0.0), Vec3::new(1.0, 3.0, 0.0)]
      ),
      (
        LineType::ConnectivityEdge,
        [Vec3::new(2.0, 3.0, 0.0), Vec3::new(2.0, 4.0, 0.0)]
      ),
      (
        LineType::ConnectivityEdge,
        [Vec3::new(2.0, 4.0, 0.0), Vec3::new(2.0, 5.0, 0.0)]
      ),
      //
      (
        LineType::AgentCorridor(agent_id),
        [Vec3::new(1.75, 1.5, 0.0), Vec3::new(1.6, 4.0, 0.0)]
      ),
      //
      (
        LineType::Target(agent_id),
        [Vec3::new(3.9, 1.5, 0.0), Vec3::new(1.5, 4.5, 0.0)]
      ),
      //
      (
        LineType::Waypoint(agent_id),
        [Vec3::new(3.9, 1.5, 0.0), Vec3::new(2.0, 3.0, 0.0)]
      ),
    ]
    .iter()
    .copied()
    .map(|(t, [p0, p1])| (t, [p0 + TRANSLATION, p1 + TRANSLATION]))
    .collect::<Vec<_>>()
  );
  assert_eq!(
    fake_drawer.triangles,
    [
      (
        TriangleType::Node,
        [
          Vec3::new(0.0, 1.0, 0.0),
          Vec3::new(1.0, 0.0, 0.0),
          Vec3::new(1.75, 1.5, 0.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(0.0, 2.0, 0.0),
          Vec3::new(0.0, 1.0, 0.0),
          Vec3::new(1.75, 1.5, 0.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(1.0, 0.0, 0.0),
          Vec3::new(2.0, 0.0, 0.0),
          Vec3::new(1.75, 1.5, 0.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(1.0, 3.0, 0.0),
          Vec3::new(0.0, 2.0, 0.0),
          Vec3::new(1.75, 1.5, 0.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(1.0, 3.0, 0.0),
          Vec3::new(2.0, 3.0, 0.0),
          Vec3::new(1.6, 4.0, 0.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(1.0, 5.0, 0.0),
          Vec3::new(1.0, 3.0, 0.0),
          Vec3::new(1.6, 4.0, 0.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(2.0, 0.0, 0.0),
          Vec3::new(4.0, 1.0, 0.0),
          Vec3::new(1.75, 1.5, 0.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(2.0, 3.0, 0.0),
          Vec3::new(1.0, 3.0, 0.0),
          Vec3::new(1.75, 1.5, 0.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(2.0, 3.0, 0.0),
          Vec3::new(2.0, 4.0, 0.0),
          Vec3::new(1.6, 4.0, 0.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(2.0, 3.0, 0.0),
          Vec3::new(3.0, 3.0, -2.0),
          Vec3::new(2.5, 3.5, -1.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(2.0, 4.0, 0.0),
          Vec3::new(2.0, 3.0, 0.0),
          Vec3::new(2.5, 3.5, -1.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(2.0, 4.0, 0.0),
          Vec3::new(2.0, 5.0, 0.0),
          Vec3::new(1.6, 4.0, 0.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(2.0, 4.0, 0.0),
          Vec3::new(3.0, 4.0, 1.0),
          Vec3::new(2.5, 4.5, 0.5)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(2.0, 5.0, 0.0),
          Vec3::new(1.0, 5.0, 0.0),
          Vec3::new(1.6, 4., 0.00)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(2.0, 5.0, 0.0),
          Vec3::new(2.0, 4.0, 0.0),
          Vec3::new(2.5, 4.5, 0.5)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(3.0, 3.0, -2.0),
          Vec3::new(3.0, 4.0, -2.0),
          Vec3::new(2.5, 3.5, -1.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(3.0, 4.0, -2.0),
          Vec3::new(2.0, 4.0, 0.0),
          Vec3::new(2.5, 3.5, -1.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(3.0, 4.0, 1.0),
          Vec3::new(3.0, 5.0, 1.0),
          Vec3::new(2.5, 4.5, 0.5)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(3.0, 5.0, 1.0),
          Vec3::new(2.0, 5.0, 0.0),
          Vec3::new(2.5, 4.5, 0.5)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(4.0, 1.0, 0.0),
          Vec3::new(4.0, 2.0, 0.0),
          Vec3::new(1.75, 1.5, 0.0)
        ]
      ),
      (
        TriangleType::Node,
        [
          Vec3::new(4.0, 2.0, 0.0),
          Vec3::new(2.0, 3.0, 0.0),
          Vec3::new(1.75, 1.5, 0.0)
        ]
      ),
    ]
    .iter()
    .copied()
    .map(|(t, [p0, p1, p2])| (
      t,
      [p0 + TRANSLATION, p1 + TRANSLATION, p2 + TRANSLATION]
    ))
    .collect::<Vec<_>>()
  );
}

#[test]
fn draws_boundary_links() {
  let nav_mesh = Arc::new(
    NavigationMesh::<XYZ> {
      vertices: vec![
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(2.0, 1.0, 1.0),
        Vec3::new(2.0, 2.0, 1.0),
        Vec3::new(1.0, 2.0, 1.0),
      ],
      polygons: vec![vec![0, 1, 2, 3]],
    }
    .validate()
    .expect("The mesh is valid."),
  );

  let mut archipelago = Archipelago::new();
  let island_id_1 = archipelago.add_island();
  let island_id_2 = archipelago.add_island();
  archipelago
    .get_island_mut(island_id_1)
    .set_nav_mesh(Transform::default(), nav_mesh.clone());
  archipelago.get_island_mut(island_id_2).set_nav_mesh(
    Transform { translation: Vec3::new(1.0, 0.0, 0.0), rotation: 0.0 },
    nav_mesh.clone(),
  );

  // Update so everything is in sync.
  archipelago.update(1.0);

  let mut fake_drawer = FakeDrawer::new();
  draw_archipelago_debug(&archipelago, &mut fake_drawer);
  fake_drawer.sort();

  let lines = fake_drawer
    .lines
    .iter()
    .filter(|(line_type, _)| *line_type == LineType::BoundaryLink)
    .map(|(_, edge)| *edge)
    .collect::<Vec<_>>();

  assert_eq!(lines, [[Vec3::new(2.0, 1.0, 1.0), Vec3::new(2.0, 2.0, 1.0)]]);
}

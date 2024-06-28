use std::sync::Arc;

use bevy::prelude::*;
use landmass::NavigationMesh;

use crate::{
  Agent, AgentBundle, AgentDesiredVelocity, AgentState, AgentTarget,
  Archipelago, ArchipelagoRef, Island, IslandBundle, LandmassPlugin, NavMesh,
};

#[test]
fn computes_path_for_agent_and_updates_desired_velocity() {
  let mut app = App::new();

  app
    .add_plugins(MinimalPlugins)
    .add_plugins(TransformPlugin)
    .add_plugins(AssetPlugin::default())
    .add_plugins(LandmassPlugin);

  let archipelago_id = app.world.spawn(Archipelago::new()).id();

  let nav_mesh = Arc::new(
    NavigationMesh {
      mesh_bounds: None,
      vertices: vec![
        landmass::Vec3::new(1.0, 0.0, 1.0),
        landmass::Vec3::new(4.0, 0.0, 1.0),
        landmass::Vec3::new(4.0, 0.0, 4.0),
        landmass::Vec3::new(3.0, 0.0, 4.0),
        landmass::Vec3::new(3.0, 0.0, 2.0),
        landmass::Vec3::new(1.0, 0.0, 2.0),
      ],
      polygons: vec![vec![0, 1, 4, 5], vec![1, 2, 3, 4]],
    }
    .validate()
    .expect("is valid"),
  );

  let nav_mesh_handle = app
    .world
    .resource::<Assets<NavMesh>>()
    .get_handle_provider()
    .reserve_handle()
    .typed::<NavMesh>();

  app
    .world
    .spawn(TransformBundle {
      local: Transform::from_translation(Vec3::new(1.0, 1.0, 1.0)),
      ..Default::default()
    })
    .insert(IslandBundle {
      island: Island,
      archipelago_ref: ArchipelagoRef(archipelago_id),
      nav_mesh: Default::default(),
    })
    .insert(nav_mesh_handle.clone());

  app
    .world
    .resource_mut::<Assets<NavMesh>>()
    .insert(nav_mesh_handle, NavMesh(nav_mesh));

  let agent_id = app
    .world
    .spawn(TransformBundle {
      local: Transform::from_translation(Vec3::new(2.5, 1.0, 2.5)),
      ..Default::default()
    })
    .insert(AgentBundle {
      agent: Agent { radius: 0.5, max_velocity: 1.0 },
      archipelago_ref: ArchipelagoRef(archipelago_id),
      target: AgentTarget::Point(Vec3::new(4.5, 1.0, 4.5)),
      velocity: Default::default(),
      state: Default::default(),
      desired_velocity: Default::default(),
    })
    .id();

  // The first update propagates the global transform, and sets the start of
  // the delta time (in this update, delta time is 0).
  app.update();
  // The second update allows landmass to update properly.
  app.update();

  assert_eq!(
    *app.world.get::<AgentState>(agent_id).expect("current state was added"),
    AgentState::Moving,
  );
  assert_eq!(
    app
      .world
      .get::<AgentDesiredVelocity>(agent_id)
      .expect("desired velocity was added")
      .0,
    Vec3::new(1.5, 0.0, 0.5).normalize(),
  );
}

#[test]
fn adds_and_removes_agents() {
  let mut app = App::new();

  app
    .add_plugins(MinimalPlugins)
    .add_plugins(AssetPlugin::default())
    .add_plugins(LandmassPlugin);

  let archipelago_id = app.world.spawn(Archipelago::new()).id();

  let agent_id_1 = app
    .world
    .spawn(TransformBundle::default())
    .insert(AgentBundle {
      agent: Agent { radius: 0.5, max_velocity: 1.0 },
      archipelago_ref: ArchipelagoRef(archipelago_id),
      target: AgentTarget::None,
      velocity: Default::default(),
      state: Default::default(),
      desired_velocity: Default::default(),
    })
    .id();

  let agent_id_2 = app
    .world
    .spawn(TransformBundle::default())
    .insert(AgentBundle {
      agent: Agent { radius: 0.5, max_velocity: 1.0 },
      archipelago_ref: ArchipelagoRef(archipelago_id),
      target: AgentTarget::None,
      velocity: Default::default(),
      state: Default::default(),
      desired_velocity: Default::default(),
    })
    .id();

  app.update();

  let archipelago =
    app.world.get::<Archipelago>(archipelago_id).expect("archipelago exists");

  fn sorted(mut v: Vec<Entity>) -> Vec<Entity> {
    v.sort();
    v
  }

  assert_eq!(
    sorted(archipelago.agents.keys().copied().collect()),
    sorted(vec![agent_id_1, agent_id_2]),
  );
  assert_eq!(archipelago.archipelago.get_agent_ids().len(), 2);

  let agent_id_3 = app
    .world
    .spawn(TransformBundle::default())
    .insert(AgentBundle {
      agent: Agent { radius: 0.5, max_velocity: 1.0 },
      archipelago_ref: ArchipelagoRef(archipelago_id),
      target: AgentTarget::None,
      velocity: Default::default(),
      state: Default::default(),
      desired_velocity: Default::default(),
    })
    .id();

  app.update();

  let archipelago =
    app.world.get::<Archipelago>(archipelago_id).expect("archipelago exists");

  assert_eq!(
    sorted(archipelago.agents.keys().copied().collect()),
    sorted(vec![agent_id_1, agent_id_2, agent_id_3]),
  );
  assert_eq!(archipelago.archipelago.get_agent_ids().len(), 3);

  app.world.despawn(agent_id_2);

  app.update();

  let archipelago =
    app.world.get::<Archipelago>(archipelago_id).expect("archipelago exists");

  assert_eq!(
    sorted(archipelago.agents.keys().copied().collect()),
    sorted(vec![agent_id_1, agent_id_3]),
  );
  assert_eq!(archipelago.archipelago.get_agent_ids().len(), 2);

  app.world.despawn(agent_id_1);
  app.world.despawn(agent_id_3);

  app.update();

  let archipelago =
    app.world.get::<Archipelago>(archipelago_id).expect("archipelago exists");

  assert_eq!(archipelago.agents.keys().copied().collect::<Vec<_>>(), []);
  assert_eq!(archipelago.archipelago.get_agent_ids().len(), 0);
}

#[test]
fn adds_and_removes_islands() {
  let mut app = App::new();

  app
    .add_plugins(MinimalPlugins)
    .add_plugins(AssetPlugin::default())
    .add_plugins(LandmassPlugin);

  let archipelago_id = app.world.spawn(Archipelago::new()).id();

  let island_id_1 = app
    .world
    .spawn(TransformBundle::default())
    .insert(IslandBundle {
      island: Island,
      archipelago_ref: ArchipelagoRef(archipelago_id),
      nav_mesh: Default::default(),
    })
    .id();

  let island_id_2 = app
    .world
    .spawn(TransformBundle::default())
    .insert(IslandBundle {
      island: Island,
      archipelago_ref: ArchipelagoRef(archipelago_id),
      nav_mesh: Default::default(),
    })
    .id();

  app.update();

  let archipelago =
    app.world.get::<Archipelago>(archipelago_id).expect("archipelago exists");

  fn sorted(mut v: Vec<Entity>) -> Vec<Entity> {
    v.sort();
    v
  }

  assert_eq!(
    sorted(archipelago.islands.keys().copied().collect()),
    sorted(vec![island_id_1, island_id_2]),
  );
  assert_eq!(archipelago.archipelago.get_island_ids().len(), 2);

  let island_id_3 = app
    .world
    .spawn(TransformBundle::default())
    .insert(IslandBundle {
      island: Island,
      archipelago_ref: ArchipelagoRef(archipelago_id),
      nav_mesh: Default::default(),
    })
    .id();

  app.update();

  let archipelago =
    app.world.get::<Archipelago>(archipelago_id).expect("archipelago exists");

  assert_eq!(
    sorted(archipelago.islands.keys().copied().collect()),
    sorted(vec![island_id_1, island_id_2, island_id_3])
  );
  assert_eq!(archipelago.archipelago.get_island_ids().len(), 3);

  app.world.despawn(island_id_2);

  app.update();

  let archipelago =
    app.world.get::<Archipelago>(archipelago_id).expect("archipelago exists");

  assert_eq!(
    sorted(archipelago.islands.keys().copied().collect()),
    sorted(vec![island_id_1, island_id_3])
  );
  assert_eq!(archipelago.archipelago.get_island_ids().len(), 2);

  app.world.despawn(island_id_1);
  app.world.despawn(island_id_3);

  app.update();

  let archipelago =
    app.world.get::<Archipelago>(archipelago_id).expect("archipelago exists");

  assert_eq!(archipelago.agents.keys().copied().collect::<Vec<_>>(), []);
  assert_eq!(archipelago.archipelago.get_agent_ids().len(), 0);
}

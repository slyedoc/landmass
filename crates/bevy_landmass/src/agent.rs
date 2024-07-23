use bevy::{
  prelude::{
    Bundle, Component, Deref, DetectChanges, Entity, Query, Ref, With,
  },
  transform::components::GlobalTransform,
  utils::HashMap,
};

use crate::{
  coords::{CoordinateSystem, ThreeD, TwoD},
  AgentState, Archipelago, TargetReachedCondition, Velocity,
};
use crate::{ArchipelagoRef, NodeType};

/// A bundle to create agents. This omits the GlobalTransform component, since
/// this is commonly added in other bundles (which is redundant and can override
/// previous bundles).
#[derive(Bundle)]
pub struct AgentBundle<CS: CoordinateSystem> {
  /// The agent itself.
  pub agent: Agent,
  /// A reference pointing to the Archipelago to associate this entity with.
  pub archipelago_ref: ArchipelagoRef<CS>,
  /// The velocity of the agent.
  pub velocity: Velocity<CS>,
  /// The target of the agent.
  pub target: AgentTarget<CS>,
  /// The current state of the agent. This is set by `landmass` (during
  /// [`LandmassSystemSet::Output`]).
  pub state: AgentState,
  /// The current desired velocity of the agent. This is set by `landmass`
  /// (during [`LandmassSystemSet::Output`]).
  pub desired_velocity: AgentDesiredVelocity<CS>,
}

pub type Agent2dBundle = AgentBundle<TwoD>;
pub type Agent3dBundle = AgentBundle<ThreeD>;

/// An agent. See [`crate::AgentBundle`] for required related components.
#[derive(Component)]
pub struct Agent {
  /// The radius of the agent.
  pub radius: f32,
  /// The speed the agent prefers to move at. This should often be set lower
  /// than the [`Self::max_speed`] to allow the agent to "speed up" in order to
  /// get out of another agent's way.
  pub desired_speed: f32,
  /// The max speed of an agent.
  pub max_speed: f32,
}

#[derive(Component, Default, Deref)]
pub struct AgentNodeTypeCostOverrides(HashMap<NodeType, f32>);

impl AgentNodeTypeCostOverrides {
  /// Sets the node type cost for this agent to `cost`. Returns false if the
  /// cost is <= 0.0. Otherwise returns true.
  pub fn set_node_type_cost(&mut self, node_type: NodeType, cost: f32) -> bool {
    if cost <= 0.0 {
      return false;
    }
    self.0.insert(node_type, cost);
    true
  }

  /// Removes the override cost for `node_type`. Returns true if `node_type` was
  /// overridden, false otherwise.
  pub fn remove_override(&mut self, node_type: NodeType) -> bool {
    self.0.remove(&node_type).is_some()
  }
}

/// The current target of the entity. Note this can be set by either reinserting
/// the component, or dereferencing:
///
/// ```rust
/// use bevy::prelude::*;
/// use bevy_landmass::AgentTarget3d;
///
/// fn clear_targets(mut targets: Query<&mut AgentTarget3d>) {
///   for mut target in targets.iter_mut() {
///     *target = AgentTarget3d::None;
///   }
/// }
/// ```
#[derive(Component)]
pub enum AgentTarget<CS: CoordinateSystem> {
  None,
  Point(CS::Coordinate),
  Entity(Entity),
}

pub type AgentTarget2d = AgentTarget<TwoD>;
pub type AgentTarget3d = AgentTarget<ThreeD>;

impl<CS: CoordinateSystem> Default for AgentTarget<CS> {
  fn default() -> Self {
    Self::None
  }
}

impl<CS: CoordinateSystem> AgentTarget<CS> {
  /// Converts an agent target to a concrete world position.
  fn to_point(
    &self,
    global_transform_query: &Query<&GlobalTransform>,
  ) -> Option<CS::Coordinate> {
    match self {
      Self::Point(point) => Some(point.clone()),
      &Self::Entity(entity) => global_transform_query
        .get(entity)
        .ok()
        .map(|transform| CS::from_bevy_position(transform.translation())),
      _ => None,
    }
  }
}

/// The current desired velocity of the agent. This is set by `landmass` (during
/// [`LandmassSystemSet::Output`]).
#[derive(Component)]
pub struct AgentDesiredVelocity<CS: CoordinateSystem>(CS::Coordinate);

pub type AgentDesiredVelocity2d = AgentDesiredVelocity<TwoD>;
pub type AgentDesiredVelocity3d = AgentDesiredVelocity<ThreeD>;

impl<CS: CoordinateSystem> Default for AgentDesiredVelocity<CS> {
  fn default() -> Self {
    Self(Default::default())
  }
}

impl<CS: CoordinateSystem> AgentDesiredVelocity<CS> {
  /// The desired velocity of the agent.
  pub fn velocity(&self) -> CS::Coordinate {
    self.0.clone()
  }
}

/// Ensures every Bevy agent has a corresponding `landmass` agent.
pub(crate) fn add_agents_to_archipelagos<CS: CoordinateSystem>(
  mut archipelago_query: Query<(Entity, &mut Archipelago<CS>)>,
  agent_query: Query<
    (Entity, &Agent, &ArchipelagoRef<CS>),
    With<GlobalTransform>,
  >,
) {
  let mut archipelago_to_agents = HashMap::<_, HashMap<_, _>>::new();
  for (entity, agent, archipleago_ref) in agent_query.iter() {
    archipelago_to_agents
      .entry(archipleago_ref.entity)
      .or_default()
      .insert(entity, agent);
  }

  for (archipelago_entity, mut archipelago) in archipelago_query.iter_mut() {
    let mut new_agent_map = archipelago_to_agents
      .remove(&archipelago_entity)
      .unwrap_or_else(HashMap::new);
    let archipelago = archipelago.as_mut();

    // Remove any agents that aren't in the `new_agent_map`. Also remove any
    // agents from the `new_agent_map` that are in the archipelago.
    archipelago.agents.retain(|agent_entity, agent_id| {
      match new_agent_map.remove(agent_entity) {
        None => {
          archipelago.archipelago.remove_agent(*agent_id);
          false
        }
        Some(_) => true,
      }
    });

    for (new_agent_entity, new_agent) in new_agent_map.drain() {
      let agent_id =
        archipelago.archipelago.add_agent(landmass::Agent::create(
          /* position= */ CS::from_landmass(&landmass::Vec3::ZERO),
          /* velocity= */ CS::from_landmass(&landmass::Vec3::ZERO),
          new_agent.radius,
          new_agent.desired_speed,
          new_agent.max_speed,
        ));
      archipelago.agents.insert(new_agent_entity, agent_id);
    }
  }
}

/// Ensures the "input state" (position, velocity, etc) of every Bevy agent
/// matches its `landmass` counterpart.
pub(crate) fn sync_agent_input_state<CS: CoordinateSystem>(
  agent_query: Query<(
    Entity,
    &Agent,
    &ArchipelagoRef<CS>,
    &GlobalTransform,
    Option<&Velocity<CS>>,
    Option<&AgentTarget<CS>>,
    Option<&TargetReachedCondition>,
    Option<Ref<AgentNodeTypeCostOverrides>>,
  )>,
  global_transform_query: Query<&GlobalTransform>,
  mut archipelago_query: Query<&mut Archipelago<CS>>,
) {
  for (
    agent_entity,
    agent,
    &ArchipelagoRef { entity: arch_entity, .. },
    transform,
    velocity,
    target,
    target_reached_condition,
    node_type_cost_overrides,
  ) in agent_query.iter()
  {
    let mut archipelago = match archipelago_query.get_mut(arch_entity) {
      Err(_) => continue,
      Ok(arch) => arch,
    };

    let landmass_agent = archipelago
      .get_agent_mut(agent_entity)
      .expect("this agent is in the archipelago");
    landmass_agent.position = CS::from_bevy_position(transform.translation());
    if let Some(Velocity { velocity }) = velocity {
      landmass_agent.velocity = velocity.clone();
    }
    landmass_agent.radius = agent.radius;
    landmass_agent.desired_speed = agent.desired_speed;
    landmass_agent.max_speed = agent.max_speed;
    landmass_agent.current_target =
      target.and_then(|target| target.to_point(&global_transform_query));
    landmass_agent.target_reached_condition =
      if let Some(target_reached_condition) = target_reached_condition {
        target_reached_condition.to_landmass()
      } else {
        landmass::TargetReachedCondition::Distance(None)
      };
    match node_type_cost_overrides {
      None => {
        for (node_type, _) in
          landmass_agent.get_node_type_cost_overrides().collect::<Vec<_>>()
        {
          landmass_agent.remove_overridden_node_type_cost(node_type);
        }
      }
      Some(node_type_cost_overrides) => {
        if !node_type_cost_overrides.is_changed() {
          continue;
        }

        for (node_type, _) in
          landmass_agent.get_node_type_cost_overrides().collect::<Vec<_>>()
        {
          if node_type_cost_overrides.0.contains_key(&node_type) {
            continue;
          }
          landmass_agent.remove_overridden_node_type_cost(node_type);
        }

        for (&node_type, &cost) in node_type_cost_overrides.0.iter() {
          assert!(landmass_agent.override_node_type_cost(node_type, cost));
        }
      }
    }
  }
}

/// Copies the agent state from `landmass` agents to their Bevy equivalent.
pub(crate) fn sync_agent_state<CS: CoordinateSystem>(
  mut agent_query: Query<
    (Entity, &ArchipelagoRef<CS>, &mut AgentState),
    With<Agent>,
  >,
  archipelago_query: Query<&Archipelago<CS>>,
) {
  for (agent_entity, &ArchipelagoRef { entity: arch_entity, .. }, mut state) in
    agent_query.iter_mut()
  {
    let archipelago = match archipelago_query.get(arch_entity).ok() {
      None => continue,
      Some(arch) => arch,
    };

    *state = AgentState::from_landmass(
      &archipelago
        .get_agent(agent_entity)
        .expect("the agent is in the archipelago")
        .state(),
    );
  }
}

/// Copies the agent desired velocity from `landmass` agents to their Bevy
/// equivalent.
pub(crate) fn sync_desired_velocity<CS: CoordinateSystem>(
  mut agent_query: Query<
    (Entity, &ArchipelagoRef<CS>, &mut AgentDesiredVelocity<CS>),
    With<Agent>,
  >,
  archipelago_query: Query<&Archipelago<CS>>,
) {
  for (
    agent_entity,
    &ArchipelagoRef { entity: arch_entity, .. },
    mut desired_velocity,
  ) in agent_query.iter_mut()
  {
    let archipelago = match archipelago_query.get(arch_entity).ok() {
      None => continue,
      Some(arch) => arch,
    };

    desired_velocity.0 = archipelago
      .get_agent(agent_entity)
      .expect("the agent is in the archipelago")
      .get_desired_velocity()
      .clone();
  }
}

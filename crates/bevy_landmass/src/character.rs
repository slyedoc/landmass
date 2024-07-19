use bevy::{
  prelude::{Bundle, Component, Entity, Query, With},
  transform::components::GlobalTransform,
  utils::hashbrown::HashMap,
};

use crate::{
  coords::{CoordinateSystem, ThreeD, TwoD},
  Archipelago, ArchipelagoRef,
};

/// A bundle to create characters. This omits the GlobalTransform component,
/// since this is commonly added in other bundles (which is redundant and can
/// override previous bundles).
#[derive(Bundle)]
pub struct CharacterBundle<CS: CoordinateSystem> {
  /// The character itself.
  pub character: Character,
  /// A reference pointing to the Archipelago to associate this entity with.
  pub archipelago_ref: ArchipelagoRef<CS>,
  /// The velocity of the character.
  pub velocity: Velocity<CS>,
}

pub type Character2dBundle = CharacterBundle<TwoD>;
pub type Character3dBundle = CharacterBundle<ThreeD>;

/// A character. See [`crate::CharacterBundle`] for required related components.
#[derive(Component)]
pub struct Character {
  /// The radius of the character.
  pub radius: f32,
}

/// The current velocity of the agent/character. This must be set to match
/// whatever speed the agent/character is going.
#[derive(Component)]
pub struct Velocity<CS: CoordinateSystem> {
  pub velocity: CS::Coordinate,
  // This can't be a tuple struct due to https://github.com/rust-lang/rust/issues/73191
}

pub type Velocity2d = Velocity<TwoD>;
pub type Velocity3d = Velocity<ThreeD>;

impl<CS: CoordinateSystem> Default for Velocity<CS> {
  fn default() -> Self {
    Self { velocity: Default::default() }
  }
}

/// Ensures every Bevy character has a corresponding `landmass` character.
pub(crate) fn add_characters_to_archipelago<CS: CoordinateSystem>(
  mut archipelagos: Query<(Entity, &mut Archipelago<CS>)>,
  characters: Query<
    (Entity, &Character, &ArchipelagoRef<CS>),
    With<GlobalTransform>,
  >,
) {
  let mut archipelago_to_characters = HashMap::<_, HashMap<_, _>>::new();
  for (entity, character, archipleago_ref) in characters.iter() {
    archipelago_to_characters
      .entry(archipleago_ref.entity)
      .or_default()
      .insert(entity, character);
  }
  for (entity, mut archipelago) in archipelagos.iter_mut() {
    let mut new_character_map =
      archipelago_to_characters.remove(&entity).unwrap_or_else(HashMap::new);
    let archipelago = archipelago.as_mut();

    archipelago.characters.retain(|character_entity, character_id| {
      if new_character_map.remove(character_entity).is_none() {
        archipelago.archipelago.remove_character(*character_id);
        false
      } else {
        true
      }
    });

    for (new_character_entity, new_character) in new_character_map.drain() {
      let character_id =
        archipelago.archipelago.add_character(landmass::Character {
          position: Default::default(),
          velocity: Default::default(),
          radius: new_character.radius,
        });
      archipelago.characters.insert(new_character_entity, character_id);
    }
  }
}

/// Copies Bevy character states to their associated landmass character.
pub(crate) fn sync_character_state<CS: CoordinateSystem>(
  characters: Query<(
    Entity,
    &Character,
    &ArchipelagoRef<CS>,
    &GlobalTransform,
    Option<&Velocity<CS>>,
  )>,
  mut archipelagos: Query<&mut Archipelago<CS>>,
) {
  for (
    character_entity,
    character,
    &ArchipelagoRef { entity: arch_entity, .. },
    transform,
    velocity,
  ) in characters.iter()
  {
    let Ok(mut archipelago) = archipelagos.get_mut(arch_entity) else {
      continue;
    };

    let landmass_character = archipelago
      .get_character_mut(character_entity)
      .expect("the characters is in the archipelago");
    landmass_character.position =
      CS::from_transform_position(transform.translation());
    landmass_character.velocity = if let Some(Velocity { velocity }) = velocity
    {
      velocity.clone()
    } else {
      CS::Coordinate::default()
    };
    landmass_character.radius = character.radius;
  }
}

//! The minimap, top right: the ring, its pillars, every fighter (green friends, red foes, your
//! target outlined) and an arrow for you. Fixed with the enemy gate up, or turning with you so you
//! always face up (the button under it, or the Keybinds screen).

use bevy::prelude::*;

use crate::fight::Fighter;
use crate::settings::Settings;
use crate::{Phase, Selection, Step};

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, build).add_systems(Update, (spawn_dots, place, toggle).in_set(Step::Show));
}

/// The map's edge in pixels.
const SIZE: f32 = 180.0;
const DOT: f32 = 9.0;
const ARROW: f32 = 18.0;

#[derive(Component)]
struct MapRoot;
#[derive(Component)]
struct Pillar(usize);
#[derive(Component)]
struct Dot(Entity);
#[derive(Component)]
struct Arrow;
#[derive(Component)]
struct RotateButton;

fn build(mut commands: Commands) {
    commands
        .spawn((
            MapRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(14.0),
                top: Val::Px(14.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|col| {
            col.spawn((
                Node {
                    width: Val::Px(SIZE),
                    height: Val::Px(SIZE),
                    border: UiRect::all(Val::Px(3.0)),
                    border_radius: BorderRadius::MAX,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.42, 0.33, 0.24, 0.85)),
                BorderColor::all(Color::srgb(0.75, 0.6, 0.3)),
            ))
            .with_children(|map| {
                for i in 0..8 {
                    map.spawn((
                        Pillar(i),
                        Node { position_type: PositionType::Absolute, border_radius: BorderRadius::MAX, display: Display::None, ..default() },
                        BackgroundColor(Color::srgb(0.2, 0.17, 0.15)),
                    ));
                }
                // You: a gold dot with a nose toward where you face.
                map.spawn((
                    Arrow,
                    Node { position_type: PositionType::Absolute, width: Val::Px(ARROW), height: Val::Px(ARROW), ..default() },
                    UiTransform::default(),
                    GlobalZIndex(5),
                ))
                .with_children(|a| {
                    a.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(ARROW / 2.0 - 2.0),
                            top: Val::Px(0.0),
                            width: Val::Px(4.0),
                            height: Val::Px(ARROW / 2.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(1.0, 0.85, 0.3)),
                    ));
                    a.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(ARROW / 2.0 - 5.0),
                            top: Val::Px(ARROW / 2.0 - 5.0),
                            width: Val::Px(10.0),
                            height: Val::Px(10.0),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(1.0, 0.85, 0.3)),
                    ));
                });
            });
            col.spawn((
                RotateButton,
                Button,
                Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                BackgroundColor(Color::srgba(0.06, 0.05, 0.05, 0.8)),
                children![(Text::new(""), TextFont { font_size: 12.0, ..default() }, TextColor(Color::srgb(0.85, 0.8, 0.7)))],
            ));
        });
}

fn spawn_dots(mut commands: Commands, added: Query<(Entity, &Fighter), Added<Fighter>>, maps: Query<&Children, With<MapRoot>>) {
    let Some(&map) = maps.iter().next().and_then(|c| c.first()) else { return };
    for (e, f) in &added {
        if f.player {
            continue;
        }
        let dot = commands
            .spawn((
                crate::MatchEntity,
                Dot(e),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(DOT),
                    height: Val::Px(DOT),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(Color::WHITE),
                BorderColor::all(Color::BLACK),
            ))
            .id();
        commands.entity(map).add_child(dot);
    }
}

/// Map space: `up` along the map's up direction (the way you face when it turns, else toward the
/// enemy gate), `right` along its right, scaled to pixels from the map's centre.
fn project(p: Vec3, fwd: Vec2, scale: f32) -> Vec2 {
    let up = p.x * fwd.x + p.z * fwd.y;
    let right = -p.x * fwd.y + p.z * fwd.x;
    Vec2::new(SIZE / 2.0 + right * scale, SIZE / 2.0 - up * scale)
}

#[allow(clippy::type_complexity)]
fn place(
    phase: Res<State<Phase>>,
    sel: Res<Selection>,
    settings: Res<Settings>,
    fighters: Query<(Entity, &Fighter, &Transform)>,
    mut root: Query<&mut Node, (With<MapRoot>, Without<Dot>, Without<Pillar>, Without<Arrow>)>,
    mut pillars: Query<(&Pillar, &mut Node), (Without<Dot>, Without<Arrow>)>,
    mut dots: Query<(Entity, &Dot, &mut Node, &mut BackgroundColor, &mut BorderColor), (Without<Pillar>, Without<Arrow>)>,
    mut arrow: Query<(&mut Node, &mut UiTransform), (With<Arrow>, Without<Dot>, Without<Pillar>)>,
    mut commands: Commands,
) {
    let on = phase.get().in_match();
    if let Ok(mut n) = root.single_mut() {
        let d = if on { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
    let Some((_, me, my_tf)) = fighters.iter().find(|(_, f, _)| f.player) else { return };
    let arena = sel.arena();
    let scale = (SIZE / 2.0 - 6.0) / arena.radius;
    let facing = {
        let f = my_tf.rotation * Vec3::Z;
        Vec2::new(f.x, f.z).normalize_or(Vec2::Y)
    };
    // Turning, the map's up is where you face; fixed, it's toward the enemy gate (+Z for gold).
    let up = if settings.minimap_rotates {
        facing
    } else if me.team == 0 {
        Vec2::Y
    } else {
        -Vec2::Y
    };
    for (p, mut n) in &mut pillars {
        match arena.pillars.get(p.0) {
            Some(&([x, z], r)) => {
                let c = project(Vec3::new(x, 0.0, z), up, scale);
                let d = (r * 2.0 * scale).max(5.0);
                n.left = Val::Px(c.x - d / 2.0);
                n.top = Val::Px(c.y - d / 2.0);
                n.width = Val::Px(d);
                n.height = Val::Px(d);
                n.display = Display::Flex;
            }
            None => n.display = Display::None,
        }
    }
    for (e, dot, mut n, mut bg, mut border) in &mut dots {
        let Ok((_, f, tf)) = fighters.get(dot.0) else {
            commands.entity(e).despawn();
            continue;
        };
        let c = project(tf.translation, up, scale);
        n.left = Val::Px(c.x - DOT / 2.0);
        n.top = Val::Px(c.y - DOT / 2.0);
        bg.0 = if f.unit.dead {
            Color::srgb(0.45, 0.45, 0.45)
        } else if f.team == me.team {
            Color::srgb(0.25, 0.9, 0.3)
        } else {
            Color::srgb(0.95, 0.2, 0.15)
        };
        *border = BorderColor::all(if me.target == Some(dot.0) { Color::WHITE } else { Color::BLACK });
    }
    if let Ok((mut n, mut t)) = arrow.single_mut() {
        let c = project(my_tf.translation, up, scale);
        n.left = Val::Px(c.x - ARROW / 2.0);
        n.top = Val::Px(c.y - ARROW / 2.0);
        // Your facing in map space, as a turn clockwise from up.
        // The map's right in world terms, as `project` lays it out.
        let right = Vec2::new(-up.y, up.x);
        let angle = facing.dot(right).atan2(facing.dot(up));
        t.rotation = Rot2::radians(angle);
    }
}

fn toggle(
    clicks: Query<&Interaction, (Changed<Interaction>, With<RotateButton>)>,
    buttons: Query<&Children, With<RotateButton>>,
    mut texts: Query<&mut Text>,
    mut settings: ResMut<Settings>,
) {
    if clicks.iter().any(|i| *i == Interaction::Pressed) {
        settings.minimap_rotates = !settings.minimap_rotates;
        settings.save();
    }
    if !settings.is_changed() {
        return;
    }
    let label = if settings.minimap_rotates { "Map: turns with you" } else { "Map: fixed" };
    for children in &buttons {
        for &c in children {
            if let Ok(mut t) = texts.get_mut(c) {
                t.0 = label.into();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_map_puts_what_is_ahead_above_you() {
        // Facing +Z (the gold team's view at the gates): ahead is up, your right is right.
        let up = Vec2::Y;
        let ahead = project(Vec3::new(0.0, 0.0, 10.0), up, 1.0);
        assert!(ahead.y < SIZE / 2.0 && (ahead.x - SIZE / 2.0).abs() < 1e-4);
        // The screen's right when facing +Z is world -X (see `input::move_player`).
        let right = project(Vec3::new(-10.0, 0.0, 0.0), up, 1.0);
        assert!(right.x > SIZE / 2.0);
        // Turned to face +X, what's at +X is up.
        let ahead = project(Vec3::new(10.0, 0.0, 0.0), Vec2::X, 1.0);
        assert!(ahead.y < SIZE / 2.0 && (ahead.x - SIZE / 2.0).abs() < 1e-4);
    }
}

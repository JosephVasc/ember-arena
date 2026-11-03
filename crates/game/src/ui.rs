//! The screens: the class, bracket and arena pick; in a match, unit frames (you, your target, your
//! party, the enemy team), the action bar, your cast bar, error text, the gate countdown, and a
//! nameplate over every fighter; then pause and the results.

use bevy::prelude::*;
use rules::spells::{Aim, AuraKind, Class, Resource, Special, Spell, CLASSES};
use rules::unit::Fail;

use crate::camera::MainCam;
use crate::fight::{Fighter, Match, Release, situation};
use crate::icons::{self, Icons};
use crate::fx;
use crate::input::{AUTO_SLOT, Errors, Press};
use crate::settings::{Action, OpenSettings, Settings, short};
use crate::{MatchEntity, Phase, Selection, Step};

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, build_screens).add_systems(
        Update,
        (show_screens, menu_input, refresh_menu, frames, action_bar, slot_feedback, slot_clicks, banners, plates, plate_controls, refresh_results)
            .in_set(Step::Show),
    );
}

const PANEL: Color = Color::srgba(0.06, 0.05, 0.05, 0.86);
const EDGE: Color = Color::srgb(0.55, 0.42, 0.22);
const GOLD: Color = Color::srgb(1.0, 0.8, 0.35);
const DIM: Color = Color::srgb(0.65, 0.62, 0.58);
const PICKED: Color = Color::srgba(0.45, 0.28, 0.1, 0.95);
const IDLE: Color = Color::srgba(0.14, 0.11, 0.1, 0.95);
const HP_FRIEND: Color = Color::srgb(0.2, 0.75, 0.25);
const HP_FOE: Color = Color::srgb(0.8, 0.15, 0.12);
const CAST: Color = Color::srgb(1.0, 0.75, 0.2);

#[derive(Component)]
struct LoadingRoot;
#[derive(Component)]
struct MenuRoot;
#[derive(Component)]
struct HudRoot;
#[derive(Component)]
struct PauseRoot;
#[derive(Component)]
struct ResultsRoot;
#[derive(Component)]
struct ResultsText;
#[derive(Component)]
struct ClassInfo;
#[derive(Component)]
struct ErrorText;
#[derive(Component)]
struct CenterText;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum MenuButton {
    Class(usize),
    Bracket(usize),
    Arena(usize),
    Enter,
}

/// Whose frame a widget shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Who {
    Player,
    Target,
    Party(usize),
    Enemy(usize),
    /// A nameplate's fighter.
    Plate(Entity),
}

/// What part of a frame a widget is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    Root,
    Name,
    Hp,
    HpText,
    Res,
    Cast,
    CastText,
    Status,
}

#[derive(Component)]
struct Bind(Who, Part);

/// An action bar slot (`usize::MAX` for the trinket), its cooldown shade, and its can't-use veil.
#[derive(Component)]
struct Slot(usize);
#[derive(Component)]
struct SlotShade(usize);
#[derive(Component)]
struct SlotVeil(usize);
#[derive(Component)]
struct SlotTip;
/// A slot's icon.
#[derive(Component)]
struct SlotIcon(usize);
/// The bright flash over a slot when its ability goes off.
#[derive(Component)]
struct SlotFlash(usize);
/// One ability's row on the menu.
#[derive(Component)]
struct SpellRow(usize);
#[derive(Component)]
struct SpellRowIcon(usize);
/// A slot's key label, following the bindings.
#[derive(Component)]
struct SlotKey(usize);

const TRINKET: usize = usize::MAX;

fn label(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont { font_size: size, ..default() },
        TextColor(color),
        TextShadow { offset: Vec2::splat(1.5), color: Color::BLACK },
    )
}

fn panel(node: Node) -> impl Bundle {
    (
        Node { border: UiRect::all(Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..node },
        BackgroundColor(PANEL),
        BorderColor::all(EDGE),
    )
}

fn full(display: Display) -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        display,
        ..default()
    }
}

fn button(action: MenuButton, title: &str, sub: &str) -> impl Bundle {
    (
        action,
        Button,
        Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(IDLE),
        BorderColor::all(EDGE),
        children![label(title.to_string(), 18.0, GOLD), label(sub.to_string(), 12.0, DIM)],
    )
}

/// A bar: a dark track with a fill bound to `who`'s `part`, and an optional text over it.
fn bar(p: &mut ChildSpawnerCommands, who: Who, part: Part, w: f32, h: f32, color: Color, text: Option<Part>) {
    p.spawn((
        Node {
            width: Val::Px(w),
            height: Val::Px(h),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
    ))
    .with_children(|b| {
        b.spawn((
            Bind(who, part),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(color),
        ));
        if let Some(t) = text {
            b.spawn((Bind(who, t), label("", (h - 4.0).max(9.0), Color::WHITE)));
        }
    });
}

/// A unit frame: name, health, resource, cast bar and a status line.
fn frame(p: &mut ChildSpawnerCommands, who: Who, w: f32) {
    p.spawn((
        Bind(who, Part::Root),
        panel(Node { flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(6.0)), row_gap: Val::Px(3.0), ..default() }),
    ))
    .with_children(|f| {
        f.spawn((Bind(who, Part::Name), label("", 15.0, GOLD)));
        bar(f, who, Part::Hp, w, 18.0, HP_FRIEND, Some(Part::HpText));
        bar(f, who, Part::Res, w, 7.0, Color::srgb(0.25, 0.45, 1.0), None);
        bar(f, who, Part::Cast, w, 12.0, CAST, Some(Part::CastText));
        f.spawn((Bind(who, Part::Status), label("", 13.0, Color::srgb(1.0, 0.45, 0.35))));
    });
}

fn build_screens(mut commands: Commands) {
    commands.spawn((
        LoadingRoot,
        Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..full(Display::Flex) },
        children![label("Loading the arena...", 28.0, GOLD)],
    ));

    // --- The pick. ---
    commands
        .spawn((MenuRoot, Node { padding: UiRect::all(Val::Px(24.0)), justify_content: JustifyContent::SpaceBetween, ..full(Display::None) }))
        .with_children(|root| {
            root.spawn(panel(Node { flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(14.0)), row_gap: Val::Px(6.0), width: Val::Px(360.0), ..default() }))
                .with_children(|col| {
                    col.spawn(label("EMBER ARENA", 38.0, GOLD));
                    col.spawn(label("Class", 17.0, Color::WHITE));
                    for (i, c) in CLASSES.iter().enumerate() {
                        let sub = format!("{} - {}", role(c), c.resource.name());
                        col.spawn(button(MenuButton::Class(i), c.name, &sub));
                    }
                    col.spawn(label("Bracket", 17.0, Color::WHITE));
                    col.spawn(Node { column_gap: Val::Px(6.0), ..default() }).with_children(|row| {
                        for n in 1..=3 {
                            row.spawn(button(MenuButton::Bracket(n), &format!("{n}v{n}"), "arena"));
                        }
                    });
                    col.spawn(label("Arena", 17.0, Color::WHITE));
                    for (i, a) in rules::arena::ARENAS.iter().enumerate() {
                        col.spawn(button(MenuButton::Arena(i), a.name, a.blurb));
                    }
                    col.spawn(button(MenuButton::Enter, "Enter the Arena", "or press Enter"));
                    col.spawn((OpenSettings, Button, Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(IDLE), BorderColor::all(EDGE), children![label("Keybinds & settings", 16.0, Color::WHITE)]));
                });
            root.spawn(panel(Node { flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(14.0)), width: Val::Px(520.0), align_self: AlignSelf::FlexStart, ..default() }))
                .with_children(|col| {
                    col.spawn((ClassInfo, label("", 15.0, Color::WHITE)));
                    for i in 0..12 {
                        col.spawn(Node { column_gap: Val::Px(8.0), margin: UiRect::top(Val::Px(4.0)), align_items: AlignItems::Center, ..default() })
                            .with_children(|r| {
                                r.spawn((SpellRowIcon(i), Node { width: Val::Px(34.0), height: Val::Px(34.0), flex_shrink: 0.0, ..default() }, ImageNode::default()));
                                r.spawn((SpellRow(i), label("", 13.0, Color::WHITE)));
                            });
                    }
                    col.spawn((label("Q. Trinket: breaks every control, once every 2 minutes.", 13.0, DIM), Node { margin: UiRect::top(Val::Px(6.0)), ..default() }));
                });
        });

    // --- The match HUD. ---
    commands
        .spawn((HudRoot, Node { ..full(Display::None) }))
        .with_children(|root| {
            // You and your target, top left; your party under you.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(12.0),
                column_gap: Val::Px(12.0),
                align_items: AlignItems::FlexStart,
                ..default()
            })
                .with_children(|row| {
                    row.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() }).with_children(|col| {
                        frame(col, Who::Player, 220.0);
                        frame(col, Who::Party(0), 160.0);
                        frame(col, Who::Party(1), 160.0);
                    });
                    frame(row, Who::Target, 220.0);
                });
            // The enemy team, top right.
            root.spawn(Node { position_type: PositionType::Absolute, right: Val::Px(12.0), top: Val::Px(240.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })
                .with_children(|col| {
                    for i in 0..3 {
                        frame(col, Who::Enemy(i), 180.0);
                    }
                });
            root.spawn((
                ErrorText,
                Node { position_type: PositionType::Absolute, top: Val::Px(110.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
                children![(label("", 20.0, Color::srgb(1.0, 0.2, 0.15)), ErrorText)],
            ));
            root.spawn((
                Node { position_type: PositionType::Absolute, top: Val::Percent(30.0), width: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
                children![(label("", 34.0, GOLD), CenterText, TextLayout::new_with_justify(Justify::Center))],
            ));
            // Your cast bar and the action bar, bottom centre.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(14.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|col| {
                col.spawn((SlotTip, label("", 14.0, Color::srgb(0.95, 0.92, 0.85)), TextLayout::new_with_justify(Justify::Center)));
                bar(col, Who::Player, Part::Cast, 300.0, 16.0, CAST, Some(Part::CastText));
                col.spawn(Node { column_gap: Val::Px(5.0), ..default() }).with_children(|row| {
                    for i in 0..10 {
                        slot(row, i);
                    }
                    slot(row, AUTO_SLOT);
                    slot(row, TRINKET);
                    slot(row, 10);
                    slot(row, 11);
                });
            });
        });

    commands.spawn((
        PauseRoot,
        Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, flex_direction: FlexDirection::Column, ..full(Display::None) },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        GlobalZIndex(10),
        children![
            label("PAUSED", 44.0, GOLD),
            label("Esc to fight on   M to leave the arena", 18.0, DIM),
            (OpenSettings, Button, Node { margin: UiRect::top(Val::Px(12.0)), padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(IDLE), BorderColor::all(EDGE), children![label("Keybinds & settings", 16.0, Color::WHITE)]),
        ],
    ));
    commands.spawn((
        ResultsRoot,
        Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..full(Display::None) },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        GlobalZIndex(10),
        children![(ResultsText, label("", 20.0, Color::WHITE), TextLayout::new_with_justify(Justify::Center))],
    ));
}

fn slot(row: &mut ChildSpawnerCommands, i: usize) {
    row.spawn((
        Slot(i),
        Button,
        Node {
            width: Val::Px(54.0),
            height: Val::Px(54.0),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(IDLE),
        BorderColor::all(EDGE),
        UiTransform::default(),
    ))
    .with_children(|s| {
        let cover = || Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
        s.spawn((SlotIcon(i), cover(), ImageNode::default()));
        s.spawn((SlotVeil(i), Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::NONE)));
        s.spawn((SlotShade(i), Node { position_type: PositionType::Absolute, bottom: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(0.0), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7))));
        s.spawn((SlotFlash(i), cover(), BackgroundColor(Color::NONE)));
        s.spawn((SlotKey(i), Node { position_type: PositionType::Absolute, top: Val::Px(1.0), right: Val::Px(3.0), ..default() }, label("", 13.0, Color::WHITE)));
    });
}

fn role(c: &Class) -> &'static str {
    match c.role {
        rules::spells::Role::Melee => "Melee",
        rules::spells::Role::Ranged => "Ranged",
        rules::spells::Role::Healer => "Healer",
    }
}

/// What an ability does, in a tooltip's voice.
pub fn describe(s: &Spell) -> String {
    let mut parts: Vec<String> = Vec::new();
    let cast = if s.cast > 0.0 { format!("{}s cast", s.cast) } else { "Instant".into() };
    let range = match s.aim {
        Aim::Myself => "self".to_string(),
        Aim::Around => format!("{} yd around you", s.radius),
        Aim::Party => "your whole team".to_string(),
        _ if s.range <= 6.0 => "melee".into(),
        _ => format!("{} yd", s.range),
    };
    let mut head = format!("{cast}, {range}");
    if s.cooldown > 0.0 {
        head += &format!(", {}s cooldown", s.cooldown);
    }
    if s.cost > 0.0 {
        head += &format!(", costs {}", s.cost);
    }
    if !s.gcd {
        head += ", off the global cooldown";
    }
    if s.free {
        head += ", usable while stunned";
    }
    if s.damage > 0.0 {
        parts.push(format!("deals {} damage", s.damage));
    }
    if s.heal > 0.0 {
        parts.push(format!("heals {}", s.heal));
    }
    if s.gain > 0.0 {
        parts.push(format!("builds {} rage", s.gain));
    }
    if let Some(a) = s.aura {
        parts.push(match a.kind {
            AuraKind::Dot(d) => format!("{d} damage a second for {}s", a.secs),
            AuraKind::Hot(h) => format!("heals {h} a second for {}s", a.secs),
            AuraKind::Absorb(p) => format!("absorbs {p} damage for {}s", a.secs),
            AuraKind::Taken(m) if a.secs > 600.0 => format!("cuts damage taken by {}%", ((1.0 - m) * 100.0).round()),
            AuraKind::Taken(m) => format!("cuts damage taken by {}% for {}s", ((1.0 - m) * 100.0).round(), a.secs),
            AuraKind::Dealt(m) if a.secs > 600.0 => format!("raises damage by {}%", ((m - 1.0) * 100.0).round()),
            AuraKind::Dealt(m) => format!("raises damage by {}% for {}s", ((m - 1.0) * 100.0).round(), a.secs),
            AuraKind::Health(m) => format!("raises maximum health by {}%", ((m - 1.0) * 100.0).round()),
            AuraKind::Crit(c) => format!("raises critical strike chance by {}%", (c * 100.0).round()),
            AuraKind::Regen(r) => format!("restores {r} more mana a second"),
        });
    }
    if let Some(c) = s.ctl {
        parts.push(match c {
            rules::control::Ctl::Slow(m, t) => format!("slows {}% for {t}s", ((1.0 - m) * 100.0).round()),
            rules::control::Ctl::Interrupt(t) => format!("interrupts and locks out casting for {t}s"),
            c => format!("{} {}s", c.name().to_lowercase(), c.secs().unwrap_or(0.0)),
        });
    }
    match s.special {
        Special::Blink(d) => parts.push(format!("blinks {d} yd forward, breaking stuns and roots")),
        Special::Leap(d) => parts.push(format!("leaps {d} yd back")),
        Special::Charge => parts.push("runs to the target at speed; roots stop it".into()),
        Special::LeapTo => parts.push("leaps to the target; roots stop it".into()),
        Special::Behind => parts.push("steps behind the target".into()),
        Special::Immune(t) => parts.push(format!("immune to everything for {t}s, breaking all controls")),
        Special::Cleanse => parts.push("removes fears, roots, silences, incapacitates and damage over time".into()),
        Special::Shatter(m) => parts.push(format!("{m}x damage on a rooted or stunned target")),
        Special::Execute(f) => parts.push(format!("finishes a target below {}% health", (f * 100.0).round())),
        Special::Drain(f) => parts.push(format!("heals you for {}% of the damage", (f * 100.0).round())),
        Special::None => {}
    }
    format!("{} ({head}): {}.", s.name, parts.join(", "))
}

#[allow(clippy::type_complexity)]
fn show_screens(
    phase: Res<State<Phase>>,
    mut roots: ParamSet<(
        Query<&mut Node, With<LoadingRoot>>,
        Query<&mut Node, With<MenuRoot>>,
        Query<&mut Node, With<HudRoot>>,
        Query<&mut Node, With<PauseRoot>>,
        Query<&mut Node, With<ResultsRoot>>,
    )>,
) {
    let p = *phase.get();
    let set = |mut n: Mut<Node>, on: bool| {
        let d = if on { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    };
    for n in &mut roots.p0() {
        set(n, p == Phase::Loading);
    }
    for n in &mut roots.p1() {
        set(n, p == Phase::Menu);
    }
    for n in &mut roots.p2() {
        set(n, p.in_match());
    }
    for n in &mut roots.p3() {
        set(n, p == Phase::Paused);
    }
    for n in &mut roots.p4() {
        set(n, p == Phase::Results);
    }
}

fn menu_input(
    phase: Res<State<Phase>>,
    keys: Res<ButtonInput<KeyCode>>,
    presses: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    mut sel: ResMut<Selection>,
    mut next: ResMut<NextState<Phase>>,
    overlay: Res<crate::settings::Overlay>,
) {
    if *phase.get() != Phase::Menu || overlay.open {
        return;
    }
    let mut enter = keys.just_pressed(KeyCode::Enter);
    for (i, b) in &presses {
        if *i != Interaction::Pressed {
            continue;
        }
        match *b {
            MenuButton::Class(c) => sel.class = c,
            MenuButton::Bracket(n) => sel.bracket = n,
            MenuButton::Arena(a) => sel.arena = a,
            MenuButton::Enter => enter = true,
        }
    }
    if enter {
        next.set(Phase::Gates);
    }
}

#[allow(clippy::type_complexity)]
fn refresh_menu(
    sel: Res<Selection>,
    icons: Res<Icons>,
    mut buttons: Query<(&MenuButton, &mut BackgroundColor)>,
    mut info: Query<&mut Text, (With<ClassInfo>, Without<SpellRow>)>,
    mut rows: Query<(&SpellRow, &mut Text), Without<ClassInfo>>,
    mut pics: Query<(&SpellRowIcon, &mut ImageNode)>,
) {
    if !sel.is_changed() {
        return;
    }
    for (b, mut bg) in &mut buttons {
        let on = match *b {
            MenuButton::Class(c) => c == sel.class,
            MenuButton::Bracket(n) => n == sel.bracket,
            MenuButton::Arena(a) => a == sel.arena,
            MenuButton::Enter => false,
        };
        bg.0 = if on { PICKED } else { IDLE };
    }
    let c = sel.class();
    if let Ok(mut t) = info.single_mut() {
        **t = format!("{} - {} - {} health\n{}\n{}", c.name, c.resource.name(), c.health, c.blurb, auto_text(c.auto));
    }
    // The ten abilities, then the two buffs.
    let spell = |i: usize| if i < 10 { &c.spells[i] } else { &c.buffs[i - 10] };
    for (r, mut t) in &mut rows {
        let tag = if r.0 < 10 { format!("{}", (r.0 + 1) % 10) } else { "Buff".to_string() };
        t.0 = format!("{tag}. {}", describe(spell(r.0)));
    }
    for (r, mut img) in &mut pics {
        img.image = icons.get(spell(r.0).key);
    }
}

/// Which fighter each frame shows this moment.
fn resolve_who(who: Who, fighters: &Query<(Entity, &Fighter, &GlobalTransform)>) -> Option<Entity> {
    let player = fighters.iter().find(|(_, f, _)| f.player);
    let side = |team: usize, skip_player: bool| {
        let mut v: Vec<(usize, Entity)> = fighters
            .iter()
            .filter(|(_, f, _)| f.team == team && !(skip_player && f.player))
            .map(|(e, f, _)| (f.slot, e))
            .collect();
        v.sort();
        v.into_iter().map(|x| x.1).collect::<Vec<_>>()
    };
    match who {
        Who::Player => player.map(|x| x.0),
        Who::Target => player.and_then(|(_, f, _)| f.target),
        Who::Party(i) => side(player.map_or(0, |p| p.1.team), true).get(i).copied(),
        Who::Enemy(i) => side(1 - player.map_or(0, |p| p.1.team), false).get(i).copied(),
        Who::Plate(e) => Some(e),
    }
}

#[allow(clippy::type_complexity)]
fn frames(
    fighters: Query<(Entity, &Fighter, &GlobalTransform)>,
    mut parts: Query<(&Bind, Option<&mut Node>, Option<&mut Text>, Option<&mut BackgroundColor>), Without<Plate>>,
) {
    let my_team = fighters.iter().find(|(_, f, _)| f.player).map_or(0, |(_, f, _)| f.team);
    for (bind, node, text, bg) in &mut parts {
        if matches!(bind.0, Who::Plate(_)) {
            continue;
        }
        let f = resolve_who(bind.0, &fighters).and_then(|e| fighters.get(e).ok()).map(|x| x.1);
        fill(bind.1, f, my_team, node, text, bg);
    }
}

fn fill(part: Part, f: Option<&Fighter>, my_team: usize, node: Option<Mut<Node>>, text: Option<Mut<Text>>, bg: Option<Mut<BackgroundColor>>) {
    let width = |n: Option<Mut<Node>>, frac: f32| {
        if let Some(mut n) = n {
            let w = Val::Percent((frac * 100.0).clamp(0.0, 100.0));
            if n.width != w {
                n.width = w;
            }
        }
    };
    let set_text = |t: Option<Mut<Text>>, s: String| {
        if let Some(mut t) = t {
            if t.0 != s {
                t.0 = s;
            }
        }
    };
    match (part, f) {
        (Part::Root, f) => {
            if let Some(mut n) = node {
                let d = if f.is_some() { Display::Flex } else { Display::None };
                if n.display != d {
                    n.display = d;
                }
            }
        }
        (_, None) => {}
        (Part::Name, Some(f)) => {
            let dead = if f.unit.dead { " (dead)" } else { "" };
            set_text(text, format!("{} - {}{dead}", f.name, f.unit.class.name));
        }
        (Part::Hp, Some(f)) => {
            if let Some(mut bg) = bg {
                bg.0 = if f.team == my_team { HP_FRIEND } else { HP_FOE };
            }
            width(node, f.unit.hp_frac());
        }
        (Part::HpText, Some(f)) => set_text(text, format!("{} / {}", f.unit.hp.round(), f.unit.max_hp.round())),
        (Part::Res, Some(f)) => {
            if let Some(mut bg) = bg {
                bg.0 = match f.unit.class.resource {
                    Resource::Mana => Color::srgb(0.25, 0.45, 1.0),
                    Resource::Energy => Color::srgb(1.0, 0.9, 0.2),
                    Resource::Rage => Color::srgb(0.85, 0.15, 0.1),
                };
            }
            width(node, f.unit.res / rules::unit::MAX_RES);
        }
        (Part::Cast, Some(f)) => width(node, f.unit.casting.map_or(0.0, |c| 1.0 - c.left / c.total)),
        (Part::CastText, Some(f)) => set_text(text, f.unit.casting.map_or(String::new(), |c| f.unit.spell(c.slot).name.to_string())),
        (Part::Status, Some(f)) => {
            let mut s = f.unit.cc.status().map_or(String::new(), |(w, t)| format!("{w} {t:.1}s"));
            if f.unit.immune_t > 0.0 {
                s = format!("Immune {:.1}s", f.unit.immune_t);
            }
            set_text(text, s);
        }
    }
}

/// The action bar: each slot's glyph, cooldown shade (the global one too), a red veil when it
/// can't be used on the current target, and the hovered slot's tooltip.
#[allow(clippy::type_complexity)]
fn action_bar(
    sel: Res<Selection>,
    settings: Res<Settings>,
    mut keys: Query<(&SlotKey, &mut Text), Without<SlotTip>>,
    fighters: Query<(Entity, &Fighter, &Transform)>,
    hovered: Query<(&Slot, &Interaction), With<Button>>,
    mut shades: Query<(&SlotShade, &mut Node)>,
    mut veils: Query<(&SlotVeil, &mut BackgroundColor)>,
    mut tip: Query<&mut Text, (With<SlotTip>, Without<SlotKey>)>,
) {
    for (k, mut t) in &mut keys {
        let s = short(settings.key(slot_action(k.0)));
        if t.0 != s {
            t.0 = s;
        }
    }
    let Some((_, me, tf)) = fighters.iter().find(|(_, f, _)| f.player) else { return };
    let u = &me.unit;
    let target = me.target.and_then(|t| fighters.get(t).ok()).map(|(_, f, t)| (f, t));
    let s = situation(sel.arena(), (me, tf), target, false, false);
    for (shade, mut n) in &mut shades {
        let frac = if shade.0 == TRINKET {
            u.trinket_cd / rules::spells::TRINKET_COOLDOWN
        } else if shade.0 == AUTO_SLOT {
            u.swing / u.class.auto.cooldown
        } else {
            let sp = u.spell(shade.0);
            let cd = if sp.cooldown > 0.0 { u.cds[shade.0] / sp.cooldown } else { 0.0 };
            let gcd = if sp.gcd { u.gcd / rules::spells::GCD } else { 0.0 };
            cd.max(gcd)
        };
        n.height = Val::Percent((frac * 100.0).clamp(0.0, 100.0));
    }
    for (veil, mut bg) in &mut veils {
        let check = match veil.0 {
            TRINKET => Ok(()),
            AUTO_SLOT => u.check_auto(s),
            i => u.check(i, s),
        };
        let bad = matches!(
            check,
            Err(Fail::NoResource | Fail::OutOfRange | Fail::NoSight | Fail::WrongTarget | Fail::NoTarget | Fail::Held | Fail::Rooted | Fail::Silenced)
        );
        bg.0 = if bad { Color::srgba(0.6, 0.05, 0.05, 0.45) } else { Color::NONE };
    }
    let hover = hovered.iter().find(|(_, i)| **i != Interaction::None).map(|(s, _)| s.0);
    if let Ok(mut t) = tip.single_mut() {
        t.0 = match hover {
            Some(TRINKET) => "Trinket (Q): breaks every stun, fear, root, silence and slow. 2 minute cooldown.".into(),
            Some(AUTO_SLOT) => auto_text(u.class.auto),
            Some(i) => describe(u.spell(i)),
            None => String::new(),
        };
    }
}

/// The binding behind a bar slot.
fn slot_action(slot: usize) -> Action {
    match slot {
        TRINKET => Action::Trinket,
        AUTO_SLOT => Action::Attack,
        i if i >= 10 => Action::Buff(i - 10),
        i => Action::Ability(i),
    }
}

/// What a white hit does, in a tooltip's voice.
fn auto_text(a: &Spell) -> String {
    let rage = if a.gain > 0.0 { format!(", builds {} rage", a.gain) } else { String::new() };
    format!(
        "{}: {} damage every {}s{rage}, can crit for double. Toggle it, or right-click an enemy; melee abilities and shots start it too.",
        a.name, a.damage, a.cooldown
    )
}

/// How long a slot's flash lasts when its ability goes off.
const FLASH_SECS: f32 = 0.3;

/// So you can feel your buttons: a held key or click sinks its slot and lights its border, the
/// ability going off flashes it, and the one you're casting glows until it lands.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn slot_feedback(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<Settings>,
    icons: Res<Icons>,
    fighters: Query<(Entity, &Fighter)>,
    mut fired: MessageReader<Release>,
    mut last_fired: Local<std::collections::HashMap<usize, f32>>,
    mut last_trinket: Local<f32>,
    mut last_swing: Local<f32>,
    mut buttons: Query<(&Slot, &Interaction, &mut BorderColor, &mut UiTransform), With<Button>>,
    mut pics: Query<(&SlotIcon, &mut ImageNode)>,
    mut flashes: Query<(&SlotFlash, &mut BackgroundColor)>,
) {
    let Some((me, f)) = fighters.iter().find(|(_, f)| f.player) else { return };
    let u = &f.unit;
    let now = time.elapsed_secs();
    for r in fired.read() {
        if r.caster == me {
            last_fired.insert(r.slot, now);
        }
    }
    if u.trinket_cd > *last_trinket {
        last_fired.insert(TRINKET, now);
    }
    *last_trinket = u.trinket_cd;
    if u.swing > *last_swing {
        last_fired.insert(AUTO_SLOT, now);
    }
    *last_swing = u.swing;
    let key_down = |slot: usize| settings.pressed(&keys, slot_action(slot));
    let casting = u.casting.map(|c| c.slot);
    let mut down: Vec<usize> = Vec::new();
    for (slot, interaction, mut border, mut tf) in &mut buttons {
        let pressed = key_down(slot.0) || *interaction == Interaction::Pressed;
        if pressed {
            down.push(slot.0);
        }
        let idle = match slot.0 {
            TRINKET => GOLD,
            AUTO_SLOT if u.attacking => Color::srgb(1.0, 0.25, 0.15),
            AUTO_SLOT => EDGE,
            i => fx::color(u.spell(i).school),
        };
        let pulse = 0.5 + 0.5 * (now * 9.0).sin();
        *border = BorderColor::all(if pressed {
            Color::WHITE
        } else if casting == Some(slot.0) {
            GOLD.mix(&Color::WHITE, pulse)
        } else if *interaction == Interaction::Hovered {
            idle.lighter(0.25)
        } else {
            idle
        });
        let scale = Vec2::splat(if pressed { 0.88 } else { 1.0 });
        if tf.scale != scale {
            tf.scale = scale;
        }
    }
    for (p, mut img) in &mut pics {
        let key = match p.0 {
            TRINKET => icons::TRINKET,
            AUTO_SLOT => u.class.auto.key,
            i => u.spell(i).key,
        };
        let h = icons.get(key);
        if img.image != h {
            img.image = h;
        }
        img.color = if down.contains(&p.0) { Color::srgb(0.6, 0.6, 0.6) } else { Color::WHITE };
    }
    for (fl, mut bg) in &mut flashes {
        let age = last_fired.get(&fl.0).map_or(f32::MAX, |t| now - t);
        let a = (1.0 - age / FLASH_SECS).clamp(0.0, 1.0) * 0.8;
        bg.0 = Color::srgba(1.0, 0.92, 0.6, a);
    }
}

fn slot_clicks(slots: Query<(&Interaction, &Slot), (Changed<Interaction>, With<Button>)>, mut out: MessageWriter<Press>) {
    for (i, s) in &slots {
        if *i == Interaction::Pressed {
            out.write(Press(s.0));
        }
    }
}

fn banners(
    phase: Res<State<Phase>>,
    state: Res<Match>,
    errors: Res<Errors>,
    settings: Res<Settings>,
    mut err: Query<&mut Text, (With<ErrorText>, Without<CenterText>)>,
    mut center: Query<&mut Text, (With<CenterText>, Without<ErrorText>)>,
) {
    if let Ok(mut t) = err.single_mut() {
        t.0 = errors.0.as_ref().map_or(String::new(), |e| e.0.clone());
    }
    if let Ok(mut t) = center.single_mut() {
        t.0 = match phase.get() {
            Phase::Gates => {
                let k = |i| settings.key(Action::Buff(i)).map_or("-".to_string(), crate::settings::label);
                format!("The gates open in {}\nBuff up: {}, {}   Enter: ready", state.gates_t.ceil().max(1.0), k(0), k(1))
            }
            Phase::Fight if state.elapsed < 1.5 => "Fight!".into(),
            _ => String::new(),
        };
    }
}

/// The big icon above a nameplate while that fighter is controlled, and its timer.
#[derive(Component)]
struct PlateIcon(Entity);
#[derive(Component)]
struct PlateTime(Entity);

/// A nameplate's root, following its fighter.
#[derive(Component)]
struct Plate(Entity);

#[allow(clippy::type_complexity)]
fn plates(
    mut commands: Commands,
    cams: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    added: Query<(Entity, &Fighter), Added<Fighter>>,
    fighters: Query<(Entity, &Fighter, &GlobalTransform)>,
    mut roots: Query<(Entity, &Plate, &mut Node, &mut BorderColor)>,
    mut parts: Query<(&Bind, Option<&mut Node>, Option<&mut Text>, Option<&mut BackgroundColor>), Without<Plate>>,
) {
    for (e, f) in &added {
        if f.player {
            continue;
        }
        commands
            .spawn((
                MatchEntity,
                Plate(e),
                Node {
                    position_type: PositionType::Absolute,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(2.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    display: Display::None,
                    ..default()
                },
                BorderColor::all(Color::NONE),
                GlobalZIndex(-1),
            ))
            .with_children(|p| {
                p.spawn((
                    PlateIcon(e),
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: Val::Percent(100.0),
                        width: Val::Px(38.0),
                        height: Val::Px(38.0),
                        margin: UiRect::bottom(Val::Px(3.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        display: Display::None,
                        ..default()
                    },
                    ImageNode::default(),
                    BorderColor::all(Color::srgb(1.0, 0.3, 0.2)),
                ))
                .with_children(|i| {
                    i.spawn((PlateTime(e), label("", 15.0, Color::WHITE)));
                });
                p.spawn((Bind(Who::Plate(e), Part::Name), label("", 13.0, Color::WHITE)));
                bar(p, Who::Plate(e), Part::Hp, 90.0, 9.0, HP_FOE, None);
                bar(p, Who::Plate(e), Part::Cast, 90.0, 7.0, CAST, None);
                p.spawn((Bind(Who::Plate(e), Part::Status), label("", 11.0, Color::srgb(1.0, 0.5, 0.4))));
            });
    }
    let Ok((cam, cam_tf)) = cams.single() else { return };
    let player = fighters.iter().find(|(_, f, _)| f.player);
    let my_team = player.map_or(0, |p| p.1.team);
    let my_target = player.and_then(|p| p.1.target);
    for (e, plate, mut node, mut border) in &mut roots {
        let Ok((_, _, tf)) = fighters.get(plate.0) else {
            commands.entity(e).despawn();
            continue;
        };
        match cam.world_to_viewport(cam_tf, tf.translation() + Vec3::Y * 2.3) {
            Ok(p) => {
                node.left = Val::Px(p.x - 47.0);
                node.top = Val::Px(p.y - 30.0);
                node.display = Display::Flex;
            }
            Err(_) => node.display = Display::None,
        }
        *border = BorderColor::all(if my_target == Some(plate.0) { Color::WHITE } else { Color::NONE });
    }
    for (bind, n, t, bg) in &mut parts {
        if let Who::Plate(e) = bind.0 {
            let f = fighters.get(e).ok().map(|x| x.1);
            if bind.1 == Part::Name {
                if let (Some(f), Some(mut t)) = (f, t) {
                    let s = f.name.clone();
                    if t.0 != s {
                        t.0 = s;
                    }
                }
                continue;
            }
            fill(bind.1, f, my_team, n, t, bg);
        }
    }
}

/// Who's controlled reads from across the arena: the control's icon and timer over their plate.
fn plate_controls(
    icons: Res<Icons>,
    fighters: Query<&Fighter>,
    mut pics: Query<(&PlateIcon, &mut Node, &mut ImageNode)>,
    mut times: Query<(&PlateTime, &mut Text)>,
) {
    for (p, mut n, mut img) in &mut pics {
        let d = fighters.get(p.0).ok().and_then(|f| f.unit.loss_of_control());
        let want = if d.is_some() { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
        if let Some(d) = d {
            let h = icons.get(d.key);
            if img.image != h {
                img.image = h;
            }
        }
    }
    for (p, mut t) in &mut times {
        if let Some(d) = fighters.get(p.0).ok().and_then(|f| f.unit.loss_of_control()) {
            t.0 = format!("{:.0}", d.left.ceil());
        }
    }
}

fn refresh_results(phase: Res<State<Phase>>, state: Res<Match>, fighters: Query<&Fighter>, mut q: Query<&mut Text, With<ResultsText>>) {
    if !phase.is_changed() || *phase.get() != Phase::Results {
        return;
    }
    let Ok(mut t) = q.single_mut() else { return };
    let head = if state.won == Some(true) { "VICTORY" } else { "DEFEAT" };
    let mut rows: Vec<&Fighter> = fighters.iter().collect();
    rows.sort_by_key(|f| (f.team, f.slot));
    let mut s = format!("{head}\n{:.0} seconds\n\n", state.elapsed);
    for f in rows {
        let side = if f.team == 0 { "Gold" } else { "Red" };
        s += &format!(
            "{side}  {} ({})  {:.0} damage  {:.0} healing  {}\n",
            f.name,
            f.unit.class.name,
            f.unit.damage_done,
            f.unit.healing_done,
            if f.unit.dead { "fell" } else { "standing" }
        );
    }
    s += "\nPress Enter";
    t.0 = s;
}

//! The screens: the class and arena pick, the in-run HUD, the level-up cards, pause and results,
//! and the bars over casting rivals and champions.

use bevy::prelude::*;
use rules::kit::{self, Kind};
use rules::text;
use rules::waves;

use crate::arena::MainCam;
use crate::fx;
use crate::rivals::Foe;
use crate::run::{self, Choice, Offer, Run, Selection};
use crate::{Phase, Rng};

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, build_screens)
        .add_systems(
            Update,
            (
                show_screens,
                menu_input,
                refresh_menu,
                refresh_hud,
                rebuild_slots,
                refresh_slots,
                deal_cards.run_if(resource_changed::<Offer>),
                card_input,
                screen_keys,
                refresh_over,
                overheads,
            ),
        );
}

const PANEL: Color = Color::srgba(0.06, 0.05, 0.05, 0.86);
const EDGE: Color = Color::srgb(0.55, 0.42, 0.22);
const GOLD: Color = Color::srgb(1.0, 0.8, 0.35);
const DIM: Color = Color::srgb(0.65, 0.62, 0.58);
const PICKED: Color = Color::srgba(0.45, 0.28, 0.1, 0.95);
const IDLE: Color = Color::srgba(0.14, 0.11, 0.1, 0.95);

#[derive(Component)]
struct MenuRoot;
#[derive(Component)]
struct HudRoot;
#[derive(Component)]
struct CardsRoot;
#[derive(Component)]
struct PauseRoot;
#[derive(Component)]
struct OverRoot;
#[derive(Component)]
struct LoadingRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum MenuButton {
    Class(usize),
    Arena(usize),
    Enter,
}

#[derive(Component)]
struct ClassInfo;
#[derive(Component)]
struct HpFill;
#[derive(Component)]
struct HpText;
#[derive(Component)]
struct ShieldFill;
#[derive(Component)]
struct XpFill;
#[derive(Component)]
struct TopText;
#[derive(Component)]
struct Banner;
#[derive(Component)]
struct Status;
#[derive(Component)]
struct SlotRow;
/// One ability slot's cooldown shade, by ability index; `usize::MAX` for F, `MAX - 1` for Q.
#[derive(Component)]
struct SlotShade(usize);
#[derive(Component)]
struct CardRow;
#[derive(Component)]
struct Card(usize);
#[derive(Component)]
struct OverText;

const SLOT_F: usize = usize::MAX;
const SLOT_Q: usize = usize::MAX - 1;

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
        Node {
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..node
        },
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
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            row_gap: Val::Px(2.0),
            ..default()
        },
        BackgroundColor(IDLE),
        BorderColor::all(EDGE),
        children![label(title.to_string(), 20.0, GOLD), label(sub.to_string(), 13.0, DIM)],
    )
}

fn build_screens(mut commands: Commands) {
    commands.spawn((
        LoadingRoot,
        Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..full(Display::Flex) },
        children![label("Loading the arena…", 28.0, GOLD)],
    ));

    // --- The pick: class and arena down the left, the chosen class's kit on the right. ---
    commands
        .spawn((
            MenuRoot,
            Node {
                padding: UiRect::all(Val::Px(28.0)),
                justify_content: JustifyContent::SpaceBetween,
                ..full(Display::None)
            },
        ))
        .with_children(|root| {
            root.spawn(panel(Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(18.0)),
                row_gap: Val::Px(8.0),
                width: Val::Px(380.0),
                ..default()
            }))
            .with_children(|col| {
                col.spawn(label("EMBER ARENA", 40.0, GOLD));
                col.spawn(label("Hold the ring. Survive ten minutes.", 15.0, DIM));
                col.spawn(label("Class", 18.0, Color::WHITE));
                for (i, c) in kit::CLASSES.iter().enumerate() {
                    let specs: Vec<&str> = c.specs.iter().map(|s| s.name).collect();
                    let sub = format!("{} · {}", specs.join(", "), c.mobility.name);
                    col.spawn(button(MenuButton::Class(i), c.name, &sub));
                }
                col.spawn(label("Arena", 18.0, Color::WHITE));
                for (i, a) in waves::ARENAS.iter().enumerate() {
                    col.spawn(button(MenuButton::Arena(i), a.name, a.blurb));
                }
                col.spawn(button(MenuButton::Enter, "Enter the Arena", "or press Enter"));
            });
            root.spawn(panel(Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(18.0)),
                width: Val::Px(420.0),
                align_self: AlignSelf::FlexStart,
                ..default()
            }))
            .with_children(|col| {
                col.spawn((ClassInfo, label("", 15.0, Color::WHITE)));
            });
        });

    // --- The HUD. ---
    commands
        .spawn((HudRoot, Node { flex_direction: FlexDirection::Column, ..full(Display::None) }))
        .with_children(|root| {
            root.spawn(Node {
                padding: UiRect::all(Val::Px(12.0)),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|row| {
                row.spawn((TopText, label("", 20.0, Color::WHITE)));
                row.spawn((Banner, label("", 28.0, GOLD)));
                row.spawn(label("WASD move · F escape · Q trinket · Esc pause", 14.0, DIM));
            });
            // The bottom block: status, slots, health, experience.
            root.spawn(Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(14.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|col| {
                col.spawn((Status, label("", 24.0, Color::srgb(1.0, 0.4, 0.3))));
                col.spawn((SlotRow, Node { column_gap: Val::Px(6.0), ..default() }));
                col.spawn((
                    Node {
                        width: Val::Px(520.0),
                        height: Val::Px(22.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.12, 0.03, 0.03)),
                    BorderColor::all(EDGE),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        HpFill,
                        Node { position_type: PositionType::Absolute, left: Val::Px(0.0), height: Val::Percent(100.0), width: Val::Percent(100.0), ..default() },
                        BackgroundColor(Color::srgb(0.75, 0.12, 0.1)),
                    ));
                    bar.spawn((
                        ShieldFill,
                        Node { position_type: PositionType::Absolute, left: Val::Px(0.0), height: Val::Percent(35.0), bottom: Val::Px(0.0), width: Val::Percent(0.0), ..default() },
                        BackgroundColor(Color::srgb(0.9, 0.9, 1.0)),
                    ));
                    bar.spawn((HpText, label("", 15.0, Color::WHITE)));
                });
                col.spawn((
                    Node { width: Val::Px(520.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
                    BackgroundColor(Color::srgb(0.05, 0.08, 0.12)),
                ))
                .with_children(|bar| {
                    bar.spawn((XpFill, Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgb(0.3, 0.8, 1.0))));
                });
            });
        });

    // --- Level-up cards. ---
    commands
        .spawn((
            CardsRoot,
            Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(18.0),
                ..full(Display::None)
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn(label("LEVEL UP", 44.0, GOLD));
            root.spawn((CardRow, Node { column_gap: Val::Px(20.0), ..default() }));
            root.spawn(label("Click a card or press 1, 2, 3", 15.0, DIM));
        });

    commands.spawn((
        PauseRoot,
        Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, flex_direction: FlexDirection::Column, ..full(Display::None) },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
        GlobalZIndex(10),
        children![label("PAUSED", 44.0, GOLD), label("Esc to fight on · M to leave the arena", 18.0, DIM)],
    ));

    commands.spawn((
        OverRoot,
        Node { justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..full(Display::None) },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        GlobalZIndex(10),
        children![(OverText, label("", 22.0, Color::WHITE), TextLayout::new_with_justify(Justify::Center))],
    ));
}

#[allow(clippy::type_complexity)]
fn show_screens(
    phase: Res<State<Phase>>,
    mut roots: ParamSet<(
        Query<&mut Node, With<LoadingRoot>>,
        Query<&mut Node, With<MenuRoot>>,
        Query<&mut Node, With<HudRoot>>,
        Query<&mut Node, With<CardsRoot>>,
        Query<&mut Node, With<PauseRoot>>,
        Query<&mut Node, With<OverRoot>>,
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
        set(n, matches!(p, Phase::Playing | Phase::LevelUp | Phase::Paused));
    }
    for n in &mut roots.p3() {
        set(n, p == Phase::LevelUp);
    }
    for n in &mut roots.p4() {
        set(n, p == Phase::Paused);
    }
    for n in &mut roots.p5() {
        set(n, p == Phase::GameOver);
    }
}

fn menu_input(
    phase: Res<State<Phase>>,
    keys: Res<ButtonInput<KeyCode>>,
    presses: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    mut sel: ResMut<Selection>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<Phase>>,
) {
    if *phase.get() != Phase::Menu {
        return;
    }
    let mut enter = keys.just_pressed(KeyCode::Enter);
    for (i, b) in &presses {
        if *i != Interaction::Pressed {
            continue;
        }
        match *b {
            MenuButton::Class(c) => sel.class = c,
            MenuButton::Arena(a) => sel.arena = a,
            MenuButton::Enter => enter = true,
        }
    }
    if enter {
        run::start(&mut run, &sel);
        next.set(Phase::Playing);
    }
}

fn refresh_menu(
    sel: Res<Selection>,
    mut buttons: Query<(&MenuButton, &mut BackgroundColor)>,
    mut info: Query<&mut Text, With<ClassInfo>>,
) {
    if !sel.is_changed() {
        return;
    }
    for (b, mut bg) in &mut buttons {
        let on = match *b {
            MenuButton::Class(c) => c == sel.class,
            MenuButton::Arena(a) => a == sel.arena,
            MenuButton::Enter => false,
        };
        bg.0 = if on { PICKED } else { IDLE };
    }
    let c = sel.class();
    let stats = kit::Stats::of(c, &[], 0.0);
    let start = kit::ability(c.start);
    let mut s = format!(
        "{}\n\nStarts with {}: {}\n\nEscape (F, level {}): {} — {}\n\nCore spells: {}\n",
        c.name,
        start.name,
        text::ability(start, &kit::Owned::new(start).cast(&stats)),
        kit::MOBILITY_LEVEL,
        c.mobility.name,
        c.mobility.desc,
        c.pool.iter().map(|k| kit::ability(k).name).collect::<Vec<_>>().join(", "),
    );
    for spec in &c.specs {
        s += &format!(
            "\n{} — {}\n   {}\n",
            spec.name,
            spec.desc,
            spec.pool.iter().map(|k| kit::ability(k).name).collect::<Vec<_>>().join(", ")
        );
    }
    s += &format!("\nPick a spec at level {}.", kit::SPEC_LEVEL);
    if let Ok(mut t) = info.single_mut() {
        **t = s;
    }
}

#[allow(clippy::type_complexity)]
fn refresh_hud(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut texts: ParamSet<(
        Query<&mut Text, With<TopText>>,
        Query<&mut Text, With<Banner>>,
        Query<&mut Text, With<Status>>,
        Query<&mut Text, With<HpText>>,
    )>,
    mut fills: ParamSet<(
        Query<&mut Node, With<HpFill>>,
        Query<&mut Node, With<XpFill>>,
        Query<&mut Node, With<ShieldFill>>,
    )>,
) {
    let stats = run.stats();
    let secs = run.elapsed as u32;
    let left = waves::VICTORY_SECS as u32 - secs.min(waves::VICTORY_SECS as u32);
    let spec = run.spec.map_or(String::new(), |s| format!("{} ", s.name));
    if let Ok(mut t) = texts.p0().single_mut() {
        **t = format!(
            "{spec}{} · Level {}   {:02}:{:02}   Kills {}",
            run.class.name,
            run.level,
            left / 60,
            left % 60,
            run.kills
        );
    }
    let banner = match run.banner.as_mut() {
        Some((msg, t)) => {
            *t -= time.delta_secs();
            if *t > 0.0 { msg.clone() } else { String::new() }
        }
        None => String::new(),
    };
    if let Ok(mut t) = texts.p1().single_mut() {
        **t = banner;
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        **t = match run.cc.status() {
            Some((what, t)) if run.trinket_cd <= 0.0 => format!("{what} {t:.1}s — press Q!"),
            Some((what, t)) => format!("{what} {t:.1}s"),
            None => String::new(),
        };
    }
    if let Ok(mut t) = texts.p3().single_mut() {
        **t = format!("{} / {}", run.hp.max(0.0).round(), stats.max_hp.round());
    }
    if let Ok(mut n) = fills.p0().single_mut() {
        n.width = Val::Percent((run.hp / stats.max_hp * 100.0).clamp(0.0, 100.0));
    }
    if let Ok(mut n) = fills.p1().single_mut() {
        n.width = Val::Percent((run.xp / waves::xp_to_next(run.level) * 100.0).clamp(0.0, 100.0));
    }
    if let Ok(mut n) = fills.p2().single_mut() {
        let s = if run.shield_t > 0.0 { run.shield } else { 0.0 };
        n.width = Val::Percent((s / stats.max_hp * 100.0).clamp(0.0, 100.0));
    }
}

fn initials(name: &str) -> String {
    name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect()
}

fn slot(row: &mut ChildSpawnerCommands, index: usize, color: Color, glyph: String, key: &str) {
    row.spawn((
        Node {
            width: Val::Px(52.0),
            height: Val::Px(52.0),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(color.darker(0.35)),
        BorderColor::all(color),
    ))
    .with_children(|s| {
        s.spawn(label(glyph, 20.0, Color::WHITE));
        s.spawn((
            SlotShade(index),
            Node { position_type: PositionType::Absolute, bottom: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(0.0), ..default() },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
        ));
        s.spawn((
            Node { position_type: PositionType::Absolute, top: Val::Px(1.0), right: Val::Px(3.0), ..default() },
            label(key.to_string(), 11.0, DIM),
        ));
    });
}

/// The ability row, rebuilt when the run's abilities change: one slot each, then F and Q.
fn rebuild_slots(
    mut commands: Commands,
    run: Res<Run>,
    rows: Query<Entity, With<SlotRow>>,
    mut last: Local<Option<(u32, bool)>>,
) {
    let key = (run.version, run.has_mobility());
    if *last == Some(key) {
        return;
    }
    *last = Some(key);
    let Ok(row) = rows.single() else { return };
    commands.entity(row).despawn_related::<Children>();
    commands.entity(row).with_children(|r| {
        for (i, o) in run.abilities.iter().enumerate() {
            slot(r, i, fx::color(o.def.school), initials(o.def.name), "");
        }
        if run.has_mobility() {
            let school = kit::ability(run.class.start).school;
            slot(r, SLOT_F, fx::color(school), initials(run.class.mobility.name), "F");
        }
        slot(r, SLOT_Q, Color::srgb(0.85, 0.75, 0.5), "PT".into(), "Q");
    });
}

fn refresh_slots(run: Res<Run>, mut shades: Query<(&SlotShade, &mut Node)>) {
    let stats = run.stats();
    for (s, mut n) in &mut shades {
        let f = match s.0 {
            SLOT_F => run.mobility_cd / run.class.mobility.cooldown,
            SLOT_Q => run.trinket_cd / kit::TRINKET_COOLDOWN,
            i => run.abilities.get(i).map_or(0.0, |o| o.cd.max(0.0) / o.cooldown(&stats)),
        };
        n.height = Val::Percent((f * 100.0).clamp(0.0, 100.0));
    }
}

fn card_text(c: Choice, run: &Run) -> (String, String, String, Color) {
    let stats = run.stats();
    match c {
        Choice::New(a) => (
            a.name.into(),
            "New ability".into(),
            text::ability(a, &kit::Owned::new(a).cast(&stats)),
            fx::color(a.school),
        ),
        Choice::Rank(i) => {
            let o = run.abilities[i];
            let mut up = o;
            up.level += 1;
            let (a, b) = (o.cast(&stats), up.cast(&stats));
            let mut lines = Vec::new();
            if b.dmg > a.dmg && o.def.kind != Kind::Guard && o.def.kind != Kind::Empower {
                lines.push(format!("Power {} » {}", text::num(a.dmg), text::num(b.dmg)));
            }
            if b.dot > a.dot {
                lines.push(format!("Per second {} » {}", text::num(a.dot), text::num(b.dot)));
            }
            if b.cooldown < a.cooldown - 0.01 {
                lines.push(format!("Cooldown {}s » {}s", text::num(a.cooldown), text::num(b.cooldown)));
            }
            if b.count > a.count {
                lines.push(format!("Targets {} » {}", a.count, b.count));
            }
            if b.radius > a.radius + 0.05 {
                lines.push(format!("Area {} » {} yd", text::num(a.radius), text::num(b.radius)));
            }
            if b.duration > a.duration + 0.05 {
                lines.push(format!("Lasts {}s » {}s", text::num(a.duration), text::num(b.duration)));
            }
            (
                o.def.name.into(),
                format!("Rank {} » {}", o.level, o.level + 1),
                lines.join("\n"),
                fx::color(o.def.school),
            )
        }
        Choice::NewPassive(p) => (p.name.into(), "New talent".into(), text::stat(p.stat, p.per_level), fx::color(p.school)),
        Choice::RankPassive(i) => {
            let (p, l) = run.passives[i];
            (
                p.name.into(),
                format!("Rank {l} » {}", l + 1),
                text::stat(p.stat, p.per_level * f32::from(l + 1)),
                fx::color(p.school),
            )
        }
        Choice::Spec(s) => {
            let names: Vec<&str> = s.pool.iter().map(|k| kit::ability(k).name).collect();
            (
                s.name.into(),
                "Specialization".into(),
                format!("{}\n\nLearn {} now.\nAlso: {}.", s.desc, names[0], names[1..].join(", ")),
                Color::srgb(1.0, 0.5, 0.0),
            )
        }
        Choice::Mend => ("Mend".into(), "Restore".into(), "Everything is mastered. Restores all health.".into(), fx::HEAL),
    }
}

fn deal_cards(mut commands: Commands, offer: Res<Offer>, run: Res<Run>, rows: Query<Entity, With<CardRow>>) {
    let Ok(row) = rows.single() else { return };
    commands.entity(row).despawn_related::<Children>();
    commands.entity(row).with_children(|r| {
        for (i, &c) in offer.0.iter().enumerate() {
            let (name, tag, desc, color) = card_text(c, &run);
            r.spawn((
                Card(i),
                Button,
                Node {
                    width: Val::Px(260.0),
                    min_height: Val::Px(320.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(10.0),
                    border: UiRect::all(Val::Px(3.0)),
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.06, 0.05, 0.97)),
                BorderColor::all(color),
            ))
            .with_children(|c| {
                c.spawn(label(format!("{}", i + 1), 14.0, DIM));
                c.spawn((
                    Node {
                        width: Val::Px(72.0),
                        height: Val::Px(72.0),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(color.darker(0.3)),
                    children![label(initials(&name), 30.0, Color::WHITE)],
                ));
                c.spawn(label(name, 22.0, color.lighter(0.15)));
                c.spawn(label(tag, 14.0, GOLD));
                c.spawn((label(desc, 14.0, Color::srgb(0.92, 0.9, 0.85)), TextLayout::new_with_justify(Justify::Center)));
            });
        }
    });
}

fn card_input(
    phase: Res<State<Phase>>,
    keys: Res<ButtonInput<KeyCode>>,
    cards: Query<(&Interaction, &Card), Changed<Interaction>>,
    mut run: ResMut<Run>,
    mut offer: ResMut<Offer>,
    mut rng: ResMut<Rng>,
    mut next: ResMut<NextState<Phase>>,
) {
    if *phase.get() != Phase::LevelUp {
        return;
    }
    let mut pick = cards.iter().find(|(i, _)| **i == Interaction::Pressed).map(|(_, c)| c.0);
    for (i, k) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3].into_iter().enumerate() {
        if keys.just_pressed(k) {
            pick = Some(i);
        }
    }
    if let Some(i) = pick.filter(|&i| i < offer.0.len()) {
        run::pick(i, &mut run, &mut offer, &mut rng, &mut next);
    }
}

fn screen_keys(phase: Res<State<Phase>>, keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<Phase>>) {
    match phase.get() {
        Phase::Playing if keys.just_pressed(KeyCode::Escape) => next.set(Phase::Paused),
        Phase::Paused if keys.just_pressed(KeyCode::Escape) => next.set(Phase::Playing),
        Phase::Paused if keys.just_pressed(KeyCode::KeyM) => next.set(Phase::Menu),
        Phase::GameOver if keys.just_pressed(KeyCode::Enter) => next.set(Phase::Menu),
        _ => {}
    }
}

fn refresh_over(phase: Res<State<Phase>>, run: Res<Run>, mut q: Query<&mut Text, With<OverText>>) {
    if !phase.is_changed() || *phase.get() != Phase::GameOver {
        return;
    }
    let Ok(mut t) = q.single_mut() else { return };
    let secs = run.elapsed as u32;
    let head = if run.won { run.arena.victory.to_string() } else { "Defeated".to_string() };
    **t = format!(
        "{head}\n\n{}{} · Level {}\nSurvived {:02}:{:02} · {} kills · {} damage\n\nPress Enter",
        run.spec.map_or(String::new(), |s| format!("{} ", s.name)),
        run.class.name,
        run.level,
        secs / 60,
        secs % 60,
        run.kills,
        run.damage_done.round()
    );
}

/// A cast bar over a casting rival, and a name and health bar over a champion.
#[derive(Component)]
struct Overhead {
    foe: Entity,
}
#[derive(Component)]
struct OverheadFill;

#[allow(clippy::type_complexity)]
fn overheads(
    mut commands: Commands,
    cams: Query<(&Camera, &GlobalTransform), With<MainCam>>,
    added: Query<(Entity, &Foe), Added<Foe>>,
    foes: Query<(&GlobalTransform, &Foe)>,
    mut bars: Query<(Entity, &Overhead, &mut Node, &Children)>,
    mut fills: Query<(&mut Node, &mut BackgroundColor), (With<OverheadFill>, Without<Overhead>)>,
) {
    for (e, f) in &added {
        if f.cast.is_none() && !f.champion {
            continue;
        }
        let w = if f.champion { 120.0 } else { 60.0 };
        commands
            .spawn((
                Overhead { foe: e },
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(w),
                    height: Val::Px(if f.champion { 9.0 } else { 6.0 }),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            ))
            .with_children(|b| {
                b.spawn((OverheadFill, Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(GOLD)));
                if f.champion {
                    b.spawn((
                        Node { position_type: PositionType::Absolute, bottom: Val::Px(10.0), ..default() },
                        label(f.name.clone(), 14.0, Color::srgb(1.0, 0.45, 0.35)),
                    ));
                }
            });
    }
    let Ok((cam, cam_tf)) = cams.single() else { return };
    for (e, oh, mut node, children) in &mut bars {
        let Ok((tf, f)) = foes.get(oh.foe) else {
            commands.entity(e).despawn();
            continue;
        };
        let (frac, color, show) = if f.dead {
            (0.0, GOLD, false)
        } else if let Some(left) = f.casting {
            (1.0 - left / waves::CAST_SECS, Color::srgb(1.0, 0.75, 0.2), true)
        } else if f.champion {
            (f.hp / f.max_hp, Color::srgb(0.85, 0.15, 0.1), true)
        } else {
            (0.0, GOLD, false)
        };
        let pos = cam.world_to_viewport(cam_tf, tf.translation() + Vec3::Y * 3.0).ok();
        match pos.filter(|_| show) {
            Some(p) => {
                let w = if f.champion { 120.0 } else { 60.0 };
                node.left = Val::Px(p.x - w / 2.0);
                node.top = Val::Px(p.y);
                node.display = Display::Flex;
            }
            None => node.display = Display::None,
        }
        if let Some(&fill) = children.first() {
            if let Ok((mut n, mut bg)) = fills.get_mut(fill) {
                n.width = Val::Percent((frac * 100.0).clamp(0.0, 100.0));
                bg.0 = color;
            }
        }
    }
}

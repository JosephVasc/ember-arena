//! What's happening to whom, at a glance. Mid-screen: a big alert when you've lost control (what
//! hit you, how long, and the trinket's key if it's ready), and a line for anyone else who's
//! controlled. Top right, beside the minimap: every debuff on you, with its time left.

use bevy::prelude::*;
use rules::control::Ctl;
use rules::unit::Debuff;

use crate::fight::Fighter;
use crate::fx;
use crate::icons::Icons;
use crate::settings::{Action, Settings, label};
use crate::{Phase, Step};

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, build).add_systems(Update, (loss_of_control, others, debuffs).in_set(Step::Show));
}

const OTHER_ROWS: usize = 5;
const DEBUFFS: usize = 8;
/// Clear of the minimap's 180 + its margin.
const DEBUFF_RIGHT: f32 = 208.0;
const ALERT: Color = Color::srgb(1.0, 0.3, 0.2);

#[derive(Component)]
struct AlertRoot;
#[derive(Component)]
struct AlertIcon;
#[derive(Component)]
struct AlertTitle;
#[derive(Component)]
struct AlertLine;
#[derive(Component)]
struct AlertFill;
#[derive(Component)]
struct OtherRow(usize);
#[derive(Component)]
struct OtherIcon(usize);
#[derive(Component)]
struct OtherText(usize);
#[derive(Component)]
struct DebuffSlot(usize);
#[derive(Component)]
struct DebuffIcon(usize);
#[derive(Component)]
struct DebuffTime(usize);
#[derive(Component)]
struct DebuffTip;

fn text(size: f32, color: Color) -> impl Bundle {
    (
        Text::new(""),
        TextFont { font_size: size, ..default() },
        TextColor(color),
        TextShadow { offset: Vec2::splat(1.5), color: Color::BLACK },
    )
}

fn build(mut commands: Commands) {
    // Mid-screen, above your character.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(140.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                ..default()
            },
            GlobalZIndex(8),
            Pickable::IGNORE,
        ))
        .with_children(|col| {
            col.spawn((
                AlertRoot,
                Node {
                    column_gap: Val::Px(12.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(6.0)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.1, 0.01, 0.01, 0.82)),
                BorderColor::all(ALERT),
            ))
            .with_children(|p| {
                p.spawn((AlertIcon, Node { width: Val::Px(64.0), height: Val::Px(64.0), ..default() }, ImageNode::default()));
                p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), width: Val::Px(400.0), ..default() })
                    .with_children(|c| {
                        c.spawn((AlertTitle, text(32.0, ALERT)));
                        c.spawn((AlertLine, text(15.0, Color::WHITE)));
                        c.spawn((Node { width: Val::Percent(100.0), height: Val::Px(8.0), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7))))
                            .with_children(|b| {
                                b.spawn((AlertFill, Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(ALERT)));
                            });
                    });
            });
            for i in 0..OTHER_ROWS {
                col.spawn((
                    OtherRow(i),
                    Node {
                        column_gap: Val::Px(6.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        align_items: AlignItems::Center,
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                ))
                .with_children(|r| {
                    r.spawn((OtherIcon(i), Node { width: Val::Px(24.0), height: Val::Px(24.0), ..default() }, ImageNode::default()));
                    r.spawn((OtherText(i), text(17.0, Color::WHITE)));
                });
            }
        });

    // Top right, growing left from the minimap.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            right: Val::Px(DEBUFF_RIGHT),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|col| {
            col.spawn(Node { flex_direction: FlexDirection::RowReverse, column_gap: Val::Px(5.0), ..default() }).with_children(|row| {
                for i in 0..DEBUFFS {
                    row.spawn((
                        DebuffSlot(i),
                        Interaction::default(),
                        Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, display: Display::None, ..default() },
                    ))
                    .with_children(|s| {
                        s.spawn((
                            DebuffIcon(i),
                            Node { width: Val::Px(42.0), height: Val::Px(42.0), border: UiRect::all(Val::Px(2.0)), ..default() },
                            ImageNode::default(),
                            BorderColor::all(ALERT),
                        ));
                        s.spawn((DebuffTime(i), text(13.0, Color::WHITE)));
                    });
                }
            });
            col.spawn((DebuffTip, text(14.0, Color::srgb(1.0, 0.85, 0.7))));
        });
}

/// Reads as a player would say it: "Stunned".
fn what(d: &Debuff) -> &'static str {
    d.ctl.map_or("Damage over time", Ctl::name)
}

fn secs(t: f32) -> String {
    if t >= 10.0 { format!("{}s", t.ceil() as i32) } else { format!("{t:.1}s") }
}

#[allow(clippy::type_complexity)]
fn loss_of_control(
    phase: Res<State<Phase>>,
    icons: Res<Icons>,
    settings: Res<Settings>,
    fighters: Query<&Fighter>,
    mut root: Query<&mut Node, (With<AlertRoot>, Without<AlertFill>)>,
    mut icon: Query<&mut ImageNode, With<AlertIcon>>,
    mut title: Query<&mut Text, (With<AlertTitle>, Without<AlertLine>)>,
    mut line: Query<&mut Text, (With<AlertLine>, Without<AlertTitle>)>,
    mut fill: Query<&mut Node, (With<AlertFill>, Without<AlertRoot>)>,
) {
    let me = fighters.iter().find(|f| f.player);
    let shown = me
        .filter(|_| matches!(phase.get(), Phase::Gates | Phase::Fight | Phase::Paused))
        .and_then(|f| f.unit.loss_of_control().map(|d| (f, d)));
    let Ok(mut r) = root.single_mut() else { return };
    let want = if shown.is_some() { Display::Flex } else { Display::None };
    if r.display != want {
        r.display = want;
    }
    let Some((f, d)) = shown else { return };
    if let Ok(mut i) = icon.single_mut() {
        let h = icons.get(d.key);
        if i.image != h {
            i.image = h;
        }
    }
    if let Ok(mut t) = title.single_mut() {
        t.0 = what(&d).to_uppercase();
    }
    if let Ok(mut t) = line.single_mut() {
        let mut s = format!("{}  {}", d.name, secs(d.left));
        if f.unit.trinket_cd <= 0.0 {
            if let Some(k) = settings.key(Action::Trinket) {
                s += &format!("   press {} to break free", label(k));
            }
        }
        t.0 = s;
    }
    if let Ok(mut n) = fill.single_mut() {
        n.width = Val::Percent((d.left / d.total * 100.0).clamp(0.0, 100.0));
    }
}

/// A line for every other fighter who's controlled: your target first, then your team's trouble.
#[allow(clippy::type_complexity)]
fn others(
    phase: Res<State<Phase>>,
    icons: Res<Icons>,
    fighters: Query<(Entity, &Fighter)>,
    mut rows: Query<(&OtherRow, &mut Node)>,
    mut pics: Query<(&OtherIcon, &mut ImageNode)>,
    mut texts: Query<(&OtherText, &mut Text, &mut TextColor)>,
) {
    let me = fighters.iter().find(|(_, f)| f.player).map(|(_, f)| f);
    let mut list: Vec<(u8, usize, &Fighter, Debuff, bool)> = Vec::new();
    if let (Some(me), true) = (me, matches!(phase.get(), Phase::Gates | Phase::Fight | Phase::Paused)) {
        for (e, f) in &fighters {
            if f.player {
                continue;
            }
            if let Some(d) = f.unit.loss_of_control() {
                let is_target = me.target == Some(e);
                let order = if is_target { 0 } else if f.team == me.team { 1 } else { 2 };
                list.push((order, f.slot, f, d, is_target));
            }
        }
    }
    list.sort_by_key(|x| (x.0, x.1));
    for (r, mut n) in &mut rows {
        let want = if r.0 < list.len() { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
    for (i, mut img) in &mut pics {
        if let Some(x) = list.get(i.0) {
            let h = icons.get(x.3.key);
            if img.image != h {
                img.image = h;
            }
        }
    }
    let my_team = me.map_or(0, |f| f.team);
    for (t, mut s, mut c) in &mut texts {
        let Some((_, _, f, d, is_target)) = list.get(t.0) else { continue };
        let who = if *is_target { format!("{} (target)", f.name) } else { f.name.clone() };
        s.0 = format!("{who} - {} {}", what(d), secs(d.left));
        // Your enemies held is good news; a friend held is a problem.
        c.0 = if f.team == my_team { Color::srgb(1.0, 0.55, 0.35) } else { Color::srgb(1.0, 0.85, 0.35) };
    }
}

#[allow(clippy::type_complexity)]
fn debuffs(
    phase: Res<State<Phase>>,
    icons: Res<Icons>,
    fighters: Query<&Fighter>,
    mut slots: Query<(&DebuffSlot, &Interaction, &mut Node)>,
    mut pics: Query<(&DebuffIcon, &mut ImageNode, &mut BorderColor)>,
    mut times: Query<(&DebuffTime, &mut Text), Without<DebuffTip>>,
    mut tip: Query<&mut Text, (With<DebuffTip>, Without<DebuffTime>)>,
) {
    let list: Vec<Debuff> = if phase.get().in_match() {
        fighters.iter().find(|f| f.player).map_or(Vec::new(), |f| f.unit.debuffs())
    } else {
        Vec::new()
    };
    let mut hovered = None;
    for (s, i, mut n) in &mut slots {
        let want = if s.0 < list.len() { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
        if *i != Interaction::None {
            hovered = list.get(s.0);
        }
    }
    for (p, mut img, mut border) in &mut pics {
        let Some(d) = list.get(p.0) else { continue };
        let h = icons.get(d.key);
        if img.image != h {
            img.image = h;
        }
        // Red for a control, the school's colour for the rest.
        *border = BorderColor::all(if d.ctl.is_some() { ALERT } else { fx::color(d.school) });
    }
    for (t, mut s) in &mut times {
        if let Some(d) = list.get(t.0) {
            s.0 = secs(d.left);
        }
    }
    if let Ok(mut t) = tip.single_mut() {
        t.0 = hovered.map_or(String::new(), |d| format!("{}: {}", d.name, what(d)));
    }
}

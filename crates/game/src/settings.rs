//! Your settings: a key for every action (rebindable on the Keybinds screen, opened from the menu or
//! the pause screen) and whether the minimap turns with you. Saved to `settings.txt` under
//! `%APPDATA%\EmberArena` (or `~/.ember-arena`), so they hold between sessions.

use bevy::prelude::*;

use crate::Phase;

pub fn plugin(app: &mut App) {
    app.insert_resource(Settings::load())
        .init_resource::<Overlay>()
        .add_systems(Startup, build)
        .add_systems(Update, (open_close, clicks, capture, refresh).chain());
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Forward,
    Back,
    StrafeLeft,
    StrafeRight,
    Jump,
    NextTarget,
    TargetSelf,
    TargetParty1,
    TargetParty2,
    Trinket,
    Attack,
    Ability(usize),
    Buff(usize),
}

impl Action {
    pub fn all() -> Vec<Action> {
        let mut v = vec![
            Action::Forward,
            Action::Back,
            Action::StrafeLeft,
            Action::StrafeRight,
            Action::Jump,
            Action::NextTarget,
            Action::TargetSelf,
            Action::TargetParty1,
            Action::TargetParty2,
            Action::Trinket,
            Action::Attack,
        ];
        v.extend((0..10).map(Action::Ability));
        v.extend((0..2).map(Action::Buff));
        v
    }

    pub fn name(self) -> String {
        match self {
            Action::Forward => "Move forward".into(),
            Action::Back => "Move back".into(),
            Action::StrafeLeft => "Strafe left".into(),
            Action::StrafeRight => "Strafe right".into(),
            Action::Jump => "Jump".into(),
            Action::NextTarget => "Target next enemy".into(),
            Action::TargetSelf => "Target yourself".into(),
            Action::TargetParty1 => "Target party member 1".into(),
            Action::TargetParty2 => "Target party member 2".into(),
            Action::Trinket => "Trinket".into(),
            Action::Attack => "Auto attack on / off".into(),
            Action::Ability(i) => format!("Action button {}", i + 1),
            Action::Buff(i) => format!("Buff {}", i + 1),
        }
    }

    /// The name in the settings file.
    fn id(self) -> String {
        match self {
            Action::Ability(i) => format!("ability{}", i + 1),
            Action::Buff(i) => format!("buff{}", i + 1),
            a => format!("{a:?}").to_lowercase(),
        }
    }

    fn default_key(self) -> KeyCode {
        match self {
            Action::Forward => KeyCode::KeyW,
            Action::Back => KeyCode::KeyS,
            Action::StrafeLeft => KeyCode::KeyA,
            Action::StrafeRight => KeyCode::KeyD,
            Action::Jump => KeyCode::Space,
            Action::NextTarget => KeyCode::Tab,
            Action::TargetSelf => KeyCode::F1,
            Action::TargetParty1 => KeyCode::F2,
            Action::TargetParty2 => KeyCode::F3,
            Action::Trinket => KeyCode::KeyQ,
            Action::Attack => KeyCode::KeyR,
            Action::Ability(i) => DIGITS[i],
            Action::Buff(0) => KeyCode::KeyV,
            Action::Buff(_) => KeyCode::KeyB,
        }
    }
}

const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Digit0,
];

/// Every key a binding may use: Esc stays reserved for clearing the target and pausing.
fn bindable() -> Vec<KeyCode> {
    use KeyCode::*;
    let mut v = vec![
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP,
        KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ, F1, F2, F3, F4, F5, F6, F7, F8, F9,
        F10, F11, F12, Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7,
        Numpad8, Numpad9, Space, Tab, Backquote, Minus, Equal, BracketLeft, BracketRight, Backslash,
        Semicolon, Quote, Comma, Period, Slash, ArrowUp, ArrowDown, ArrowLeft, ArrowRight, ShiftLeft,
        ShiftRight, ControlLeft, ControlRight, AltLeft, AltRight, CapsLock, Enter, Backspace, Insert,
        Delete, Home, End, PageUp, PageDown,
    ];
    v.extend(DIGITS);
    v
}

/// A key as a player reads it: "W", "1", "Space", "Num 5".
pub fn label(k: KeyCode) -> String {
    let s = format!("{k:?}");
    if let Some(r) = s.strip_prefix("Key").or_else(|| s.strip_prefix("Digit")) {
        return r.to_string();
    }
    if let Some(r) = s.strip_prefix("Numpad") {
        return format!("Num {r}");
    }
    match k {
        KeyCode::Backquote => "`".into(),
        KeyCode::Minus => "-".into(),
        KeyCode::Equal => "=".into(),
        KeyCode::ShiftLeft => "LShift".into(),
        KeyCode::ShiftRight => "RShift".into(),
        KeyCode::ControlLeft => "LCtrl".into(),
        KeyCode::ControlRight => "RCtrl".into(),
        KeyCode::AltLeft => "LAlt".into(),
        KeyCode::AltRight => "RAlt".into(),
        _ => s,
    }
}

/// A short label for an action button's corner.
pub fn short(k: Option<KeyCode>) -> String {
    k.map_or(String::new(), |k| {
        let l = label(k);
        if l.len() > 4 { l[..4].to_string() } else { l }
    })
}

fn parse(s: &str) -> Option<KeyCode> {
    bindable().into_iter().find(|k| format!("{k:?}") == s)
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Settings {
    binds: Vec<(Action, Option<KeyCode>)>,
    /// The minimap turns so you always face up.
    pub minimap_rotates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            binds: Action::all().into_iter().map(|a| (a, Some(a.default_key()))).collect(),
            minimap_rotates: false,
        }
    }
}

impl Settings {
    pub fn key(&self, a: Action) -> Option<KeyCode> {
        self.binds.iter().find(|(x, _)| *x == a).and_then(|(_, k)| *k)
    }

    pub fn pressed(&self, keys: &ButtonInput<KeyCode>, a: Action) -> bool {
        self.key(a).is_some_and(|k| keys.pressed(k))
    }

    pub fn just(&self, keys: &ButtonInput<KeyCode>, a: Action) -> bool {
        self.key(a).is_some_and(|k| keys.just_pressed(k))
    }

    /// Bind `key` to `a`; any action that had it loses it.
    pub fn bind(&mut self, a: Action, key: KeyCode) {
        for (x, k) in &mut self.binds {
            if *k == Some(key) {
                *k = None;
            }
            if *x == a {
                *k = Some(key);
            }
        }
    }

    fn path() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("APPDATA")
            .map(|d| std::path::PathBuf::from(d).join("EmberArena"))
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".ember-arena")))?;
        Some(base.join("settings.txt"))
    }

    fn to_text(&self) -> String {
        let mut s = format!("minimap_rotates={}\n", self.minimap_rotates);
        for (a, k) in &self.binds {
            s += &format!("{}={}\n", a.id(), k.map_or("none".to_string(), |k| format!("{k:?}")));
        }
        s
    }

    fn from_text(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((name, value)) = line.split_once('=') else { continue };
            if name == "minimap_rotates" {
                s.minimap_rotates = value.trim() == "true";
                continue;
            }
            let Some(a) = Action::all().into_iter().find(|a| a.id() == name.trim()) else { continue };
            match value.trim() {
                "none" => {
                    if let Some(slot) = s.binds.iter_mut().find(|(x, _)| *x == a) {
                        slot.1 = None;
                    }
                }
                v => {
                    if let Some(k) = parse(v) {
                        s.bind(a, k);
                    }
                }
            }
        }
        s
    }

    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map_or_else(Self::default, |t| Self::from_text(&t))
    }

    pub fn save(&self) {
        let Some(p) = Self::path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&p, self.to_text()) {
            warn!("settings: couldn't save {}: {e}", p.display());
        }
    }
}

/// The Keybinds screen: open or not, and the action waiting for a key.
#[derive(Resource, Default)]
pub struct Overlay {
    pub open: bool,
    capturing: Option<Action>,
}

impl Overlay {
    /// Waiting for a key to bind: Esc cancels that rather than closing.
    pub fn capturing(&self) -> bool {
        self.capturing.is_some()
    }
}

#[derive(Component)]
struct Root;
#[derive(Component)]
struct BindButton(Action);
#[derive(Component)]
struct BindText(Action);
#[derive(Component)]
struct RotateText;
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Button2 {
    Rotate,
    Reset,
    Close,
}

/// The menu and pause screens' button that opens the Keybinds screen.
#[derive(Component)]
pub struct OpenSettings;

const GOLD: Color = Color::srgb(1.0, 0.8, 0.35);
const IDLE: Color = Color::srgba(0.14, 0.11, 0.1, 0.95);
const HOT: Color = Color::srgba(0.45, 0.28, 0.1, 0.95);

fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(s), TextFont { font_size: size, ..default() }, TextColor(color))
}

fn build(mut commands: Commands) {
    let button = |w: f32| {
        (
            Button,
            Node {
                width: Val::Px(w),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(IDLE),
            BorderColor::all(Color::srgb(0.55, 0.42, 0.22)),
        )
    };
    commands
        .spawn((
            Root,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            GlobalZIndex(30),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(4.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.05, 0.05, 0.96)),
                BorderColor::all(Color::srgb(0.55, 0.42, 0.22)),
            ))
            .with_children(|col| {
                col.spawn(text("KEYBINDS", 28.0, GOLD));
                col.spawn(text("Click an action, then press its new key. Esc cancels.", 13.0, Color::srgb(0.7, 0.67, 0.6)));
                col.spawn(Node { column_gap: Val::Px(24.0), ..default() }).with_children(|cols| {
                    let all = Action::all();
                    for half in all.chunks(10) {
                        cols.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
                            .with_children(|c| {
                                for &a in half {
                                    c.spawn(Node { column_gap: Val::Px(10.0), align_items: AlignItems::Center, ..default() })
                                        .with_children(|row| {
                                            row.spawn((text(a.name(), 15.0, Color::WHITE), Node { width: Val::Px(190.0), ..default() }));
                                            row.spawn((BindButton(a), button(110.0))).with_children(|b| {
                                                b.spawn((BindText(a), text("", 15.0, GOLD)));
                                            });
                                        });
                                }
                            });
                    }
                });
                col.spawn(Node { column_gap: Val::Px(10.0), margin: UiRect::top(Val::Px(10.0)), ..default() })
                    .with_children(|row| {
                        row.spawn((Button2::Rotate, button(220.0))).with_children(|b| {
                            b.spawn((RotateText, text("", 15.0, Color::WHITE)));
                        });
                        row.spawn((Button2::Reset, button(160.0))).with_children(|b| {
                            b.spawn(text("Reset to defaults", 15.0, Color::WHITE));
                        });
                        row.spawn((Button2::Close, button(100.0))).with_children(|b| {
                            b.spawn(text("Close", 15.0, Color::WHITE));
                        });
                    });
            });
        });
}

fn open_close(
    phase: Res<State<Phase>>,
    opens: Query<&Interaction, (Changed<Interaction>, With<OpenSettings>)>,
    mut overlay: ResMut<Overlay>,
    mut root: Query<&mut Node, With<Root>>,
) {
    if opens.iter().any(|i| *i == Interaction::Pressed) && matches!(phase.get(), Phase::Menu | Phase::Paused) {
        overlay.open = true;
        overlay.capturing = None;
    }
    if !matches!(phase.get(), Phase::Menu | Phase::Paused) {
        overlay.open = false;
    }
    if let Ok(mut n) = root.single_mut() {
        let d = if overlay.open { Display::Flex } else { Display::None };
        if n.display != d {
            n.display = d;
        }
    }
}

fn clicks(
    binds: Query<(&Interaction, &BindButton), Changed<Interaction>>,
    buttons: Query<(&Interaction, &Button2), Changed<Interaction>>,
    mut overlay: ResMut<Overlay>,
    mut settings: ResMut<Settings>,
) {
    if !overlay.open {
        return;
    }
    for (i, b) in &binds {
        if *i == Interaction::Pressed {
            overlay.capturing = Some(b.0);
        }
    }
    for (i, b) in &buttons {
        if *i != Interaction::Pressed {
            continue;
        }
        match b {
            Button2::Rotate => {
                settings.minimap_rotates = !settings.minimap_rotates;
                settings.save();
            }
            Button2::Reset => {
                let rotates = settings.minimap_rotates;
                *settings = Settings { minimap_rotates: rotates, ..Settings::default() };
                settings.save();
            }
            Button2::Close => overlay.open = false,
        }
    }
}

/// The next key pressed becomes the capturing action's.
fn capture(keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<Overlay>, mut settings: ResMut<Settings>) {
    let Some(a) = overlay.capturing else { return };
    if keys.just_pressed(KeyCode::Escape) {
        overlay.capturing = None;
        return;
    }
    let keys_ok = bindable();
    if let Some(&k) = keys.get_just_pressed().find(|k| keys_ok.contains(k)) {
        settings.bind(a, k);
        settings.save();
        overlay.capturing = None;
    }
}

fn refresh(
    overlay: Res<Overlay>,
    settings: Res<Settings>,
    mut texts: Query<(&BindText, &mut Text), Without<RotateText>>,
    mut rotate: Query<&mut Text, With<RotateText>>,
    mut backs: Query<(&BindButton, &mut BackgroundColor)>,
) {
    if !overlay.is_changed() && !settings.is_changed() {
        return;
    }
    for (b, mut t) in &mut texts {
        t.0 = if overlay.capturing == Some(b.0) {
            "press a key".into()
        } else {
            settings.key(b.0).map_or("Unbound".into(), label)
        };
    }
    for (b, mut bg) in &mut backs {
        bg.0 = if overlay.capturing == Some(b.0) { HOT } else { IDLE };
    }
    if let Ok(mut t) = rotate.single_mut() {
        t.0 = format!("Rotate minimap: {}", if settings.minimap_rotates { "On" } else { "Off" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebinding_moves_a_key_rather_than_doubling_it() {
        let mut s = Settings::default();
        s.bind(Action::Jump, KeyCode::KeyW);
        assert_eq!(s.key(Action::Jump), Some(KeyCode::KeyW));
        assert_eq!(s.key(Action::Forward), None);
    }

    #[test]
    fn settings_survive_a_save_and_load() {
        let mut s = Settings::default();
        s.bind(Action::Ability(0), KeyCode::KeyE);
        s.bind(Action::Trinket, KeyCode::Numpad5);
        s.minimap_rotates = true;
        assert_eq!(Settings::from_text(&s.to_text()), s);
    }

    #[test]
    fn keys_read_like_a_player_would_say_them() {
        assert_eq!(label(KeyCode::KeyW), "W");
        assert_eq!(label(KeyCode::Digit0), "0");
        assert_eq!(label(KeyCode::Space), "Space");
        assert_eq!(label(KeyCode::Numpad5), "Num 5");
        assert_eq!(short(Some(KeyCode::Space)), "Spac");
    }

    #[test]
    fn every_default_is_bindable_and_distinct() {
        let s = Settings::default();
        let keys: Vec<KeyCode> = Action::all().into_iter().filter_map(|a| s.key(a)).collect();
        assert_eq!(keys.len(), Action::all().len());
        for (i, k) in keys.iter().enumerate() {
            assert!(bindable().contains(k));
            assert!(!keys[i + 1..].contains(k));
        }
    }
}

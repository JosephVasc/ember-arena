//! The art: the CC0 KayKit characters and arena pieces (see `assets/licenses`), each character's
//! weapons for its loadout, and its animations, driven by what the unit is doing.

use std::time::Duration;

use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;
use rules::spells::{Gear, Model};

use crate::Phase;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load)
        .add_systems(Update, build_graphs.run_if(in_state(Phase::Loading)))
        .add_systems(Update, drive_anims);
}

pub const MODELS: [Model; 5] = [
    Model::Knight,
    Model::Barbarian,
    Model::Mage,
    Model::Rogue,
    Model::HoodedRogue,
];

fn file(model: Model) -> &'static str {
    match model {
        Model::Knight => "characters/Knight.glb",
        Model::Barbarian => "characters/Barbarian.glb",
        Model::Mage => "characters/Mage.glb",
        Model::Rogue => "characters/Rogue.glb",
        Model::HoodedRogue => "characters/Rogue_Hooded.glb",
    }
}

/// Every weapon and prop mesh a model carries; a loadout shows only some.
fn weapons(model: Model) -> &'static [&'static str] {
    match model {
        Model::Knight => &[
            "1H_Sword_Offhand",
            "Badge_Shield",
            "Rectangle_Shield",
            "Round_Shield",
            "Spike_Shield",
            "1H_Sword",
            "2H_Sword",
        ],
        Model::Barbarian => &["1H_Axe_Offhand", "Barbarian_Round_Shield", "1H_Axe", "2H_Axe", "Mug"],
        Model::Mage => &["Spellbook", "Spellbook_open", "1H_Wand", "2H_Staff"],
        Model::Rogue | Model::HoodedRogue => {
            &["Knife_Offhand", "1H_Crossbow", "2H_Crossbow", "Knife", "Throwable"]
        }
    }
}

fn shown(gear: Gear) -> &'static [&'static str] {
    match gear {
        Gear::SwordShield => &["1H_Sword", "Round_Shield"],
        Gear::GreatAxe => &["2H_Axe"],
        Gear::Staff => &["2H_Staff"],
        Gear::Wand => &["1H_Wand", "Spellbook"],
        Gear::Knives => &["Knife", "Knife_Offhand"],
        Gear::Crossbow => &["2H_Crossbow"],
    }
}

/// The clips a unit plays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Clip {
    Idle,
    Run,
    Death,
    Hit,
    Slash,
    Chop,
    Dual,
    Spin,
    Cast,
    Casting,
    Shoot,
    Cheer,
    StrafeLeft,
    StrafeRight,
    Back,
    Jump,
}

impl Clip {
    const ALL: [Clip; 16] = [
        Clip::Idle,
        Clip::Run,
        Clip::Death,
        Clip::Hit,
        Clip::Slash,
        Clip::Chop,
        Clip::Dual,
        Clip::Spin,
        Clip::Cast,
        Clip::Casting,
        Clip::Shoot,
        Clip::Cheer,
        Clip::StrafeLeft,
        Clip::StrafeRight,
        Clip::Back,
        Clip::Jump,
    ];

    fn name(self) -> &'static str {
        match self {
            Clip::Idle => "Idle",
            Clip::Run => "Running_A",
            Clip::Death => "Death_A",
            Clip::Hit => "Hit_A",
            Clip::Slash => "1H_Melee_Attack_Slice_Diagonal",
            Clip::Chop => "2H_Melee_Attack_Chop",
            Clip::Dual => "Dualwield_Melee_Attack_Slice",
            Clip::Spin => "2H_Melee_Attack_Spin",
            Clip::Cast => "Spellcast_Shoot",
            Clip::Casting => "Spellcasting",
            Clip::Shoot => "2H_Ranged_Shoot",
            Clip::Cheer => "Cheer",
            Clip::StrafeLeft => "Running_Strafe_Left",
            Clip::StrafeRight => "Running_Strafe_Right",
            Clip::Back => "Walking_Backwards",
            Clip::Jump => "Jump_Full_Short",
        }
    }

    fn looped(self) -> bool {
        matches!(
            self,
            Clip::Idle | Clip::Run | Clip::Casting | Clip::StrafeLeft | Clip::StrafeRight | Clip::Back
        )
    }

    /// The attack a loadout swings with.
    pub fn attack(gear: Gear) -> Clip {
        match gear {
            Gear::SwordShield => Clip::Slash,
            Gear::GreatAxe => Clip::Chop,
            Gear::Knives => Clip::Dual,
            Gear::Staff | Gear::Wand => Clip::Cast,
            Gear::Crossbow => Clip::Shoot,
        }
    }
}

/// One model's scene, and its graph with a node per [`Clip`].
pub struct ModelArt {
    pub scene: Handle<Scene>,
    pub gltf: Handle<Gltf>,
    pub graph: Option<Handle<AnimationGraph>>,
    pub nodes: Vec<AnimationNodeIndex>,
}

#[derive(Resource)]
pub struct Art {
    pub models: Vec<(Model, ModelArt)>,
    pub arena: ArenaArt,
}

/// The arena pieces.
pub struct ArenaArt {
    pub floor: Handle<Scene>,
    pub dirt: Handle<Scene>,
    pub wall: Handle<Scene>,
    pub wall_arched: Handle<Scene>,
    pub pillar: Handle<Scene>,
    pub pillar_grand: Handle<Scene>,
    pub torch: Handle<Scene>,
    pub banner_red: Handle<Scene>,
    pub banner_blue: Handle<Scene>,
    pub barrier: Handle<Scene>,
    pub rubble: Handle<Scene>,
    pub barrel: Handle<Scene>,
}

impl Art {
    pub fn model(&self, model: Model) -> &ModelArt {
        &self.models.iter().find(|(m, _)| *m == model).expect("every model loads").1
    }

    pub fn ready(&self) -> bool {
        self.models.iter().all(|(_, a)| a.graph.is_some())
    }
}

fn load(mut commands: Commands, assets: Res<AssetServer>) {
    let scene = |path: &str| assets.load(GltfAssetLabel::Scene(0).from_asset(path.to_string()));
    let models = MODELS
        .iter()
        .map(|&m| {
            (
                m,
                ModelArt {
                    scene: scene(file(m)),
                    gltf: assets.load(file(m)),
                    graph: None,
                    nodes: Vec::new(),
                },
            )
        })
        .collect();
    let piece = |name: &str| scene(&format!("arena/{name}.glb"));
    commands.insert_resource(Art {
        models,
        arena: ArenaArt {
            floor: piece("floor_tile_large"),
            dirt: piece("floor_dirt_large"),
            wall: piece("wall"),
            wall_arched: piece("wall_arched"),
            pillar: piece("pillar"),
            pillar_grand: piece("pillar_decorated"),
            torch: piece("torch_mounted"),
            banner_red: piece("banner_red"),
            banner_blue: piece("banner_blue"),
            barrier: piece("barrier"),
            rubble: piece("rubble_large"),
            barrel: piece("barrel_large"),
        },
    });
}

/// Once each model's file is in, a graph of the clips the game plays, by name.
fn build_graphs(
    mut art: ResMut<Art>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut next: ResMut<NextState<Phase>>,
) {
    for (model, a) in &mut art.models {
        if a.graph.is_some() {
            continue;
        }
        let Some(gltf) = gltfs.get(&a.gltf) else { continue };
        let clips = Clip::ALL.iter().map(|c| {
            gltf.named_animations
                .get(c.name())
                .unwrap_or_else(|| panic!("{model:?} has no {} animation", c.name()))
                .clone()
        });
        let (graph, nodes) = AnimationGraph::from_clips(clips);
        a.graph = Some(graphs.add(graph));
        a.nodes = nodes;
    }
    if art.ready() {
        next.set(Phase::Menu);
    }
}

/// A character's look: which model, what it carries, and the graph to animate it with.
#[derive(Component, Clone)]
pub struct Look {
    pub model: Model,
    pub gear: Gear,
}

/// What a character is doing, for its animation: a looping base and a one-shot over it.
#[derive(Component)]
pub struct Anim {
    pub base: Clip,
    pub once: Option<(Clip, f32)>,
    playing: Option<Clip>,
    /// A one-shot was asked for since the last frame: play it from the start, even over itself.
    fresh: bool,
    /// The entity holding the scene's `AnimationPlayer`, once the scene is in.
    player: Option<Entity>,
    pub dead: bool,
}

impl Anim {
    pub fn play(&mut self, clip: Clip, secs: f32) {
        if !self.dead {
            self.once = Some((clip, secs));
            self.fresh = true;
        }
    }

    pub fn die(&mut self) {
        self.dead = true;
        self.once = None;
    }
}

/// A character in the arena: its model in the loadout, animated.
pub fn character(art: &Art, model: Model, gear: Gear) -> impl Bundle {
    (
        SceneRoot(art.model(model).scene.clone()),
        Look { model, gear },
        Anim {
            base: Clip::Idle,
            once: None,
            playing: None,
            fresh: false,
            player: None,
            dead: false,
        },
    )
}

/// Wire a character's scene once it is in: hide the weapons its loadout doesn't carry, and hook
/// its animation player to the model's graph. Observes [`SceneInstanceReady`] on the root.
pub fn on_ready(
    ready: On<SceneInstanceReady>,
    mut commands: Commands,
    art: Res<Art>,
    children: Query<&Children>,
    names: Query<&Name>,
    players: Query<(), With<AnimationPlayer>>,
    mut roots: Query<(&Look, &mut Anim)>,
) {
    let root = ready.entity;
    let Ok((look, mut anim)) = roots.get_mut(root) else { return };
    let all = weapons(look.model);
    let keep = shown(look.gear);
    for e in children.iter_descendants(root) {
        if let Ok(name) = names.get(e) {
            if all.contains(&name.as_str()) && !keep.contains(&name.as_str()) {
                commands.entity(e).insert(Visibility::Hidden);
            }
        }
        if players.contains(e) && anim.player.is_none() {
            anim.player = Some(e);
            if let Some(graph) = &art.model(look.model).graph {
                commands
                    .entity(e)
                    .insert((AnimationGraphHandle(graph.clone()), AnimationTransitions::new()));
            }
        }
    }
}

fn drive_anims(
    time: Res<Time>,
    art: Res<Art>,
    mut units: Query<(&Look, &mut Anim)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let dt = time.delta_secs();
    for (look, mut anim) in &mut units {
        let anim = &mut *anim;
        if let Some((_, t)) = anim.once.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                anim.once = None;
            }
        }
        let want = if anim.dead {
            Clip::Death
        } else {
            anim.once.map_or(anim.base, |(c, _)| c)
        };
        let Some(player_e) = anim.player else { continue };
        let Ok((mut player, mut transitions)) = players.get_mut(player_e) else { continue };
        if anim.playing == Some(want) && !anim.fresh {
            continue;
        }
        anim.fresh = false;
        let nodes = &art.model(look.model).nodes;
        let Some(&node) = Clip::ALL.iter().position(|c| *c == want).and_then(|i| nodes.get(i)) else {
            continue;
        };
        let active = transitions.play(&mut player, node, Duration::from_millis(150));
        if want.looped() {
            active.repeat();
        }
        anim.playing = Some(want);
    }
}

# Ember Arena

Team arena battles, 1v1 / 2v2 / 3v3. Pick a class, target with Tab or a click, cast your ten
abilities from the action bar, play the pillars for line of sight, and be the last team standing.
Teammates and opponents are computer-controlled for now; the rules are engine-free
(`crates/rules`) so a multiplayer server can run them later.

## Run it

Double-click `play.bat`, or `cargo run -p game`. Needs Rust (the pinned toolchain installs itself).

## Controls

| Input | Action |
| --- | --- |
| W / S | Forward / back |
| A / D | Strafe left / right |
| Space | Jump (breaks a cast) |
| Right mouse drag | Turn camera and character |
| Left mouse drag | Look around freely |
| Both mouse buttons | Run forward |
| Mouse wheel | Zoom |
| Tab / click | Target an enemy / any fighter |
| F1 - F3 | Target yourself / party |
| 1 - 0 | Abilities (or click the bar) |
| R / right-click an enemy | Auto attack on / off (melee abilities and shots start it too) |
| Q | Trinket: break every control |
| V / B | Your class's two long buffs (cast them behind the gates) |
| Enter | At the gates: ready, cut the wait short |
| Esc | Clear target, then pause |

Every key can be rebound: **Keybinds & settings** on the menu or the pause screen (Esc).
Bindings save to `%APPDATA%\EmberArena\settings.txt`.

The minimap (top right) shows the ring, its pillars, friends (green), foes (red) and your
target (outlined). The button under it, or the settings screen, makes it turn with you.

## Classes

Vanguard (rage, plate), Berserker (rage, great axe), Arcanist (mana, frost and fire),
Shadowblade (energy, knives), Ranger (mana, crossbow), Mystic (mana, healer).

## Art

All 3D art is CC0 by Kay Lousberg (KayKit Character Pack: Adventurers, KayKit Dungeon
Remastered), from the official KayKit GitHub repositories; licenses in
`crates/game/assets/licenses`. Spell effects and UI are the game's own.

## Testing

`cargo test -p rules`. `EMBER_AUTO=<class> cargo run -p game` plays a match by itself and saves
screenshots (see `crates/game/src/auto.rs`).

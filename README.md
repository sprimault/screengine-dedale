# Dédale

Français : [README.fr.md](README.fr.md)

Find your way out of a maze. You pick up ammunition and healing vials, monsters
roam it, and you either put them down or lose health on contact.

**This is the demonstration game for [screengine](https://github.com/sprimault/screengine)**,
a software 3D renderer: no GPU, direct colour, lightmaps, and the same image
bit-for-bit on every target. The game exists to show what the engine does, and to
test it where its own examples cannot — by consuming it from the outside, as anyone
else would.

The aim is that of late-1990s maze games: a first-person view, dark corridors, and a
plan of what you have already walked on a key. The maze is generated, never drawn by
hand — one seed, and the same one plays again.

MIT or Apache-2.0, at your option — see [`LICENSE-MIT`](LICENSE-MIT) and
[`LICENSE-APACHE`](LICENSE-APACHE). Unless its author states otherwise, any
contribution submitted for inclusion is licensed under those same two licences.
Sprite sheets, textures and sounds are produced for the project and carry the same
licences: **nothing here comes from an existing game.** The one exception is a CC0
font, recorded in [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES).

## Status

**Step 1: the maze.** A generated maze walked in first person, floors linked by
shafts — stepped, or ramped. Not a game yet: no monsters, no shooting, no pickups.

- [`ROADMAP.md`](ROADMAP.md) — the steps and what is out of scope (French)
- [`CHANGELOG.md`](CHANGELOG.md) — what each version brought, dated
- [`docs/conception.md`](docs/conception.md) — what the game does, and who each
  piece asks: the engine, or the game (French)
- [`docs/construction.md`](docs/construction.md) — how it builds, and how the
  dependency on the engine is pinned (French)

## What the engine does not do, so the game does it itself

This list is not a complaint: it is the boundary, and it has reasons.

- **Sliding along a wall.** The engine returns a time of impact and a normal, never
  a response. Sliding, stepping and gravity are game policy.
- **Shooting a monster.** The engine stops a ray on a cell's geometry; a monster has
  no portal, hence no adjacency. Deciding it is hittable is a game rule.
- **The health bar, the score, the weapon in hand.** Everything in screen
  coordinates is drawn into the buffer after the frame ends. The engine knows no
  interface, and its line drawing takes world coordinates.
- **Sound.** The engine has neither a clock nor an audio output, and never will.

## Building

```
make build    compile
make run      run the game
make test     the tests
```

The details, and why the toolchain writes where it writes:
[`docs/construction.md`](docs/construction.md) (French).

## Controls

Left click grabs the cursor, middle click releases it, `Esc` quits.

| Key | Effect |
|---|---|
| `W` `S`, or up and down arrows | move forward, back |
| `A` `D`, or left and right arrows | turn in place |
| mouse | aim the view, once the cursor is grabbed |

**You walk, you hug walls and you fall**: walls, floor and ceiling hold, an angled step
follows the wall instead of sticking to it, and gravity brings you back down.

**Upper floors are out of reach for now**: a flight of steps stops you and a ramp slides
you back down, because the step threshold is missing. It comes next.

// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le point d'entrée du jeu.
//!
//! **Rien de ce fichier n'est définitif** : c'est le squelette qui prouve que la
//! chaîne tient — le moteur se lie, la fenêtre s'ouvre, la boucle tourne, et
//! l'interface a son tampon. Chaque étape de `ROADMAP.md` le remplace par un peu
//! plus, et `Game` part dans `game.rs` dès qu'il porte autre chose qu'un compteur.

use screengine_play::{Affine3, Color, Error, KeyCode, Output, Play, Triangle, Vec3};

/// Ce que la partie garde entre deux images.
///
/// Un compteur et rien d'autre, le temps du squelette : dès l'étape 1 elle porte
/// le labyrinthe, la pose du joueur et sa cellule, et part dans `game.rs`.
struct Game {
    /// Le nombre de pas joués, qui sert à montrer que la boucle avance.
    ticks: u64,
}

/// Les quatre sommets du panneau d'accueil, devant la caméra neutre.
const PANEL: [Vec3; 4] = [
    Vec3::new(4.0, 2.0, 1.2),
    Vec3::new(4.0, -2.0, 1.2),
    Vec3::new(4.0, -2.0, -1.2),
    Vec3::new(4.0, 2.0, -1.2),
];

/// Ses deux triangles. L'ordre des sommets décide de la face vue : la caméra
/// neutre regarde le +X, son axe droit est le −Y et son haut le +Z. Pris dans
/// l'autre sens, le panneau est un dos et l'écran reste noir — le défaut que
/// l'exemple `hello` du moteur a porté pendant plusieurs versions.
const FACES: [Triangle; 2] = [
    Triangle {
        indices: [0, 1, 2],
        color: Color::new(0x40, 0x50, 0x70, 0xFF),
    },
    Triangle {
        indices: [0, 2, 3],
        color: Color::new(0x30, 0x3C, 0x58, 0xFF),
    },
];

/// Ouvre la fenêtre ; Échap ferme.
fn main() -> Result<(), Error> {
    Play::new().title("Dédale").run_with_output(
        Game { ticks: 0 },
        |game, tick| {
            if tick.input().pressed(KeyCode::Escape) {
                tick.exit();
            }
            game.ticks += 1;
        },
        |_, context| {
            // Un refus ne peut venir que de la capacité, que cette scène
            // n'approche pas : le laisser passer vaut mieux qu'arrêter la boucle
            // sur une image manquante.
            let _ = context.submit(Affine3::IDENTITY, &PANEL, &FACES);
        },
        gauge,
    )
}

/// Une barre qui avance avec les pas joués, dans le tampon de l'hôte.
///
/// **Elle est là pour une raison, et ce n'est pas la décoration** : c'est le seul
/// endroit où une interface se dessine, et le squelette doit le prouver dès le
/// premier jour. Le moteur ne connaît ni vie, ni score, ni arme ; son tracé de
/// lignes prend des coordonnées de monde. Tout ce qui est en coordonnées d'écran
/// passe ici, après la fin d'image.
fn gauge(game: &mut Game, output: &mut Output<'_>) {
    let width = output.width().min(160);
    let filled = (game.ticks % u64::from(width)) as u32;
    for y in 0..6 {
        for x in 0..width {
            let shade = if x < filled { 0xC0 } else { 0x20 };
            if let Some(pixel) = output.pixel(8 + x, 8 + y) {
                pixel.copy_from_slice(&[shade, shade, 0x30, 0xFF]);
            }
        }
    }
}

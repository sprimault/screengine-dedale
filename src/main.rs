// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le point d'entrée du jeu.
//!
//! **Rien de ce fichier n'est définitif** : c'est le squelette qui prouve que la
//! chaîne tient — le moteur se lie, la fenêtre s'ouvre, la boucle tourne, et
//! l'interface a son tampon. Chaque étape de `ROADMAP.md` le remplace par un peu
//! plus.

mod maze;

use maze::grid::{Grid, Settings, Side};
use screengine_play::{Affine3, Color, Error, KeyCode, Output, Play, Triangle, Vec3};

/// Les réglages du labyrinthe.
///
/// Seize cases de côté sur deux étages : assez pour que la traversée du moteur
/// ait de quoi éliminer, et assez peu pour que la carte entière tienne dans la
/// capacité de triangles que la boucle impose sans qu'on puisse la relever.
const MAZE: Settings = Settings {
    extent: (16, 16, 2),
    seed: 0x5EED_1A8E,
    vertical_odds: 16,
};

/// L'état du monde : la carte, et ce qui s'en déduit.
///
/// **Il ne porte rien de la partie** — ni vie, ni score, ni pose de monstre. Le
/// monde est rechargeable à chaud, la partie est jetée au rechargement, et c'est
/// la séparation qui rend l'édition possible. L'état de partie n'a encore aucun
/// champ ; il naîtra dans `game.rs` avec le premier.
struct World {
    /// Le labyrinthe engendré.
    maze: Grid,
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
    let world = World {
        maze: Grid::generate(MAZE),
    };

    Play::new().title("Dédale").run_with_output(
        world,
        |_, tick| {
            if tick.input().pressed(KeyCode::Escape) {
                tick.exit();
            }
        },
        |_, context| {
            // Un refus ne peut venir que de la capacité, que cette scène
            // n'approche pas : le laisser passer vaut mieux qu'arrêter la boucle
            // sur une image manquante.
            let _ = context.submit(Affine3::IDENTITY, &PANEL, &FACES);
        },
        overview,
    )
}

/// Le côté d'une case dans le plan de contrôle, en pixels.
const CELL: u32 = 7;

/// La marge du plan de contrôle au coin de l'écran, en pixels.
const INSET: u32 = 8;

/// L'écart entre deux étages voisins, en pixels.
const GAP: u32 = 10;

/// Le labyrinthe dessiné à plat, un étage par plan, dans le tampon de l'hôte.
///
/// **C'est un contrôle, pas la vue de dessus du jeu.** Celle-ci se trace en
/// lignes de **monde** par le moteur, à l'étape 9 ; ce plan-ci est fait de pixels
/// posés en coordonnées d'**écran** après la fin d'image, le seul endroit où une
/// interface a sa place. Il est là parce qu'une grille ne se vérifie pas en
/// raisonnant, et qu'il valait mieux la voir avant que l'export puisse masquer
/// un défaut de génération.
///
/// **Les étages se dessinent tous, et c'est le fond de l'affaire** : un plan à un
/// seul niveau rend un labyrinthe 3D illisible, parce qu'il montre côte à côte
/// deux cases que des dizaines de passages séparent. Une case qui monte et celle
/// qui lui répond à l'étage voisin se lisent à la même position d'un plan à
/// l'autre.
fn overview(world: &mut World, output: &mut Output<'_>) {
    let maze = &world.maze;
    let (width, height, levels) = maze.extent();

    for level in 0..levels {
        let origin = INSET + level * (width * CELL + GAP);
        for y in 0..height {
            for x in 0..width {
                let cell = (x, y, level);
                // Le nord en haut de l'écran, donc la ligne s'inverse.
                let top = INSET + (height - 1 - y) * CELL;
                let left = origin + x * CELL;

                if maze.has_wall(cell, Side::North) {
                    block(output, left, top, CELL + 1, 1, WALL);
                }
                if maze.has_wall(cell, Side::South) {
                    block(output, left, top + CELL, CELL + 1, 1, WALL);
                }
                if maze.has_wall(cell, Side::West) {
                    block(output, left, top, 1, CELL + 1, WALL);
                }
                if maze.has_wall(cell, Side::East) {
                    block(output, left + CELL, top, 1, CELL + 1, WALL);
                }

                mark(output, maze, cell, left, top);
            }
        }
    }
}

/// Ce qu'une case porte au-delà de ses murs : l'entrée, la sortie, ou les
/// passages d'étage qu'elle ouvre.
///
/// **Chaque passage a sa teinte, et c'est la même de ses deux côtés.** La
/// position ne suffit pas à apparier : deux plans voisins se comparent mal à
/// l'œil, et compter des lignes sur l'un pour les retrouver sur l'autre est
/// exactement ce qu'un plan existe pour éviter. La couleur, elle, se suit d'un
/// coup d'œil.
///
/// La moitié haute d'une case dit ce qui monte, la moitié basse ce qui descend,
/// si bien qu'une case entre deux étages porte deux teintes — celle du passage
/// d'en haut et celle du passage d'en bas.
fn mark(output: &mut Output<'_>, maze: &Grid, cell: (u32, u32, u32), left: u32, top: u32) {
    let inner = CELL - 2;
    if cell == maze.start() {
        block(output, left + 1, top + 1, inner, inner, START);
        return;
    }
    if cell == maze.exit() {
        block(output, left + 1, top + 1, inner, inner, EXIT);
        return;
    }

    let half = inner / 2;
    if !maze.has_wall(cell, Side::Up) {
        block(output, left + 1, top + 1, inner, half, link(maze, cell));
    }
    if !maze.has_wall(cell, Side::Down) {
        // Un passage se nomme par sa case du dessous, des deux côtés.
        let below = (cell.0, cell.1, cell.2 - 1);
        block(
            output,
            left + 1,
            top + 1 + inner - half,
            inner,
            half,
            link(maze, below),
        );
    }
}

/// La teinte d'un passage, nommé par sa case du dessous.
///
/// Le rang se recompte à chaque image plutôt que de se ranger quelque part : ce
/// plan est un contrôle qu'on retire à l'étape 9, et lui donner un état dans le
/// monde serait lui donner plus de place qu'il n'en mérite.
fn link(maze: &Grid, below: (u32, u32, u32)) -> [u8; 4] {
    let (width, height, levels) = maze.extent();
    let mut rank = 0;
    for z in 0..levels {
        for y in 0..height {
            for x in 0..width {
                if (x, y, z) == below {
                    return LINKS[rank % LINKS.len()];
                }
                if !maze.has_wall((x, y, z), Side::Up) {
                    rank += 1;
                }
            }
        }
    }
    LINKS[rank % LINKS.len()]
}

/// La teinte d'un mur.
const WALL: [u8; 4] = [0x90, 0x94, 0xA4, 0xFF];

/// Celle de la case d'entrée.
const START: [u8; 4] = [0x40, 0xC0, 0x60, 0xFF];

/// Celle de la case de sortie.
const EXIT: [u8; 4] = [0xC8, 0x50, 0x40, 0xFF];

/// Les teintes des passages d'étage, prises à tour de rôle.
///
/// Douze, bien écartées sur le cercle : deux passages qui la partageraient sont
/// distants de douze rangs dans l'ordre de lecture, donc jamais voisins à
/// l'écran. Ni le vert de l'entrée ni le rouge de la sortie n'y figurent, et les
/// marques de case y sont pleines quand un passage n'est qu'une moitié.
const LINKS: [[u8; 4]; 12] = [
    [0xE8, 0x7A, 0x5A, 0xFF],
    [0xE8, 0xA8, 0x40, 0xFF],
    [0xD8, 0xD8, 0x48, 0xFF],
    [0x9C, 0xD0, 0x48, 0xFF],
    [0x50, 0xC8, 0x90, 0xFF],
    [0x48, 0xD0, 0xD0, 0xFF],
    [0x58, 0xA8, 0xE8, 0xFF],
    [0x78, 0x80, 0xE8, 0xFF],
    [0xA8, 0x68, 0xE0, 0xFF],
    [0xE0, 0x70, 0xC0, 0xFF],
    [0xC8, 0x98, 0x78, 0xFF],
    [0xF0, 0xF0, 0xF0, 0xFF],
];

/// Un rectangle plein, borné par le tampon.
fn block(output: &mut Output<'_>, x: u32, y: u32, width: u32, height: u32, color: [u8; 4]) {
    for row in 0..height {
        for column in 0..width {
            if let Some(pixel) = output.pixel(x + column, y + row) {
                pixel.copy_from_slice(&color);
            }
        }
    }
}

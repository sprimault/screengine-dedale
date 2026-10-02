// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le point d'entrée du jeu : les réglages, la boucle, et les trois rappels.
//!
//! **Les rappels ne portent rien.** Le pas de mise à jour délègue à l'état de la
//! partie, le rendu à `scene::submit`, et la sortie au plan de contrôle. C'est
//! voulu : la boucle ouvre une fenêtre, donc elle ne servira pas à l'image animée
//! du `README`, et tout ce qui vivrait dans un rappel serait à en sortir ce
//! jour-là.

mod game;
mod maze;
mod scene;

use game::Game;
use maze::export;
use maze::grid::{Settings, Side};
use scene::Scenery;
use screengine_play::{Error, KeyCode, Output, Play};

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

/// Ce que la boucle garde : le monde d'un côté, la partie de l'autre.
///
/// La frontière est ici et nulle part ailleurs. Le monde se recharge à chaud,
/// la partie est jetée avec lui, et rien de l'une n'entre dans l'autre.
struct Session {
    /// L'état du monde.
    scenery: Scenery,
    /// L'état de la partie.
    game: Game,
}

/// Ouvre la fenêtre ; Échap ferme.
fn main() -> Result<(), Error> {
    let scenery = Scenery::new(MAZE)?;
    let game = Game::new(scenery.entrance(), &scenery.map);

    // Le compte dans le titre, faute d'une police : c'est la seule sortie
    // textuelle du jeu avant l'étape 5, et le total de triangles est ce qu'il
    // faut surveiller — la boucle le plafonne à 16 384 sans levier.
    let title = format!(
        "Dédale — {} cellules, {} triangles",
        scenery.map.cell_count(),
        scenery.map.triangle_count()
    );

    Play::new().title(&title).run_with_output(
        Session { scenery, game },
        |session, tick| {
            if tick.input().pressed(KeyCode::Escape) {
                tick.exit();
            }
            session.game.step(tick, &session.scenery.map);
        },
        |session, context| {
            // Un refus ne vient que de la capacité de triangles, et une image
            // manquante vaut mieux qu'une boucle arrêtée. Ce que la traversée
            // rend — complète, tronquée, ou hors cellule — se relèvera à `E1.5`.
            let _ = scene::submit(context, &session.scenery, &session.game.view());
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
/// interface a sa place.
///
/// **Les étages se dessinent tous** : un plan à un seul niveau rend un labyrinthe
/// 3D illisible, parce qu'il montre côte à côte deux cases que des dizaines de
/// passages séparent.
fn overview(session: &mut Session, output: &mut Output<'_>) {
    let maze = &session.scenery.maze;
    let here = session.game.cell();
    let (width, height, levels) = maze.extent();

    // Un fond opaque sous le plan : posé à même le décor, il se confond avec
    // lui dès qu'un mur clair passe derrière.
    let span = levels * (width * CELL + GAP) - GAP;
    block(
        output,
        INSET / 2,
        INSET / 2,
        span + INSET,
        height * CELL + INSET,
        BACKDROP,
    );

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

                mark(output, session, cell, here, left, top);
            }
        }
    }
}

/// Ce qu'une case porte au-delà de ses murs : où l'on est, l'entrée, la sortie,
/// ou les passages d'étage qu'elle ouvre.
///
/// **Chaque passage a sa teinte, et c'est la même de ses deux côtés.** La
/// position ne suffit pas à apparier : deux plans voisins se comparent mal à
/// l'œil, et compter des lignes sur l'un pour les retrouver sur l'autre est
/// exactement ce qu'un plan existe pour éviter.
fn mark(
    output: &mut Output<'_>,
    session: &Session,
    cell: (u32, u32, u32),
    here: u32,
    left: u32,
    top: u32,
) {
    let maze = &session.scenery.maze;
    let inner = CELL - 2;

    if export::cell_id(maze, cell) == here {
        block(output, left + 1, top + 1, inner, inner, HERE);
        return;
    }
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
        block(output, left + 1, top + 1, inner, half, link(session, cell));
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
            link(session, below),
        );
    }
}

/// La teinte d'un passage, nommé par sa case du dessous.
///
/// Le rang se recompte à chaque image plutôt que de se ranger quelque part : ce
/// plan est un contrôle qu'on retire à l'étape 9, et lui donner un état dans le
/// monde serait lui donner plus de place qu'il n'en mérite.
fn link(session: &Session, below: (u32, u32, u32)) -> [u8; 4] {
    let maze = &session.scenery.maze;
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

/// Le fond du plan, derrière tout le reste.
const BACKDROP: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

/// La teinte d'un mur.
const WALL: [u8; 4] = [0x90, 0x94, 0xA4, 0xFF];

/// Celle de la case où l'on se trouve.
const HERE: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

/// Celle de la case d'entrée.
const START: [u8; 4] = [0x40, 0xC0, 0x60, 0xFF];

/// Celle de la case de sortie.
const EXIT: [u8; 4] = [0xC8, 0x50, 0x40, 0xFF];

/// Les teintes des passages d'étage, prises à tour de rôle.
///
/// Douze, bien écartées sur le cercle : deux passages qui la partageraient sont
/// distants de douze rangs dans l'ordre de lecture, donc jamais voisins à
/// l'écran.
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

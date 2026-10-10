// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le décor, les directions et le tampon d'écran dont plusieurs fichiers d'épreuves
//! ont besoin.
//!
//! **Il naît du partage du corps et du joueur**, qui éprouvent désormais deux
//! sujets dans deux fichiers alors qu'un seul labyrinthe de référence les sert :
//! recopier sa construction les aurait fait diverger à la première graine ajoutée,
//! et les faire dépendre l'un de l'autre aurait inversé l'ordre — les épreuves du
//! joueur auraient importé les internes de celles du corps.
//!
//! **Il ne porte que ce qui ne touche aucun interne**, et c'est ce qui trace sa
//! frontière : une fabrique de pose construit un corps par ses champs privés, donc
//! elle ne peut pas vivre ici et reste auprès du module qu'elle éprouve. Ce qui
//! monte ici est ce qui s'écrit avec l'API publique seule — une grille, une carte
//! chargée, un jeu de directions.

mod canvas;

pub use canvas::Canvas;

use std::fs::File;
use std::io::BufWriter;

use png::{BitDepth, ColorType, Encoder};
use screengine_play::{BYTES_PER_PIXEL, Config, Context, Vec3, World};

use crate::maze::export;
use crate::maze::grid::{Grid, Settings, Side};

/// La largeur des images d'épreuve.
pub const WIDTH: u32 = 320;

/// Leur hauteur.
pub const HEIGHT: u32 = 180;

/// Un contexte hors fenêtre, au budget de triangles donné.
///
/// `0` prend le défaut du moteur, qui suffit à la traversée mais **pas** au
/// chemin brut : celui-ci soumet toutes les cellules de la carte, visibles ou
/// non, et les compte toutes.
pub fn context(max_triangles: u32) -> Context {
    Context::new(Config {
        max_width: WIDTH,
        max_height: HEIGHT,
        width: WIDTH,
        height: HEIGHT,
        tile_size: 64,
        max_triangles,
        max_lines: 0,
    })
    .expect("configuration tenable")
}

/// Écrit une image rendue hors fenêtre dans `.tmp/`, pour qu'on la regarde.
///
/// **C'est un instrument, pas une épreuve.** Ce qu'une image de contrôle tranche —
/// de quel côté une silhouette regarde, si des pieds touchent le sol — ne s'écrit
/// dans aucune assertion : il faut l'œil. Ce que le rendu hors fenêtre apporte est
/// qu'on n'a plus à ouvrir une fenêtre et à attraper le bon instant.
///
/// Le chemin est dans `.tmp/`, que l'antivirus ne surveille pas et que rien ne
/// versionne.
pub fn snapshot(name: &str, pixels: &[u8]) {
    let path = format!(".tmp/{name}.png");
    let Ok(file) = File::create(&path) else {
        eprintln!("image de contrôle {path} non écrite");
        return;
    };

    let mut encoder = Encoder::new(BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    if let Ok(mut writer) = encoder.write_header() {
        let _ = writer.write_image_data(pixels);
    }
}

/// Termine l'image et rend ses pixels.
///
/// **C'est le chemin que l'étape 8 empruntera pour l'image animée**, et il est déjà
/// celui des épreuves : rien de ce qui se soumet ne lit de fenêtre, donc une scène
/// se rend ici exactement comme elle se rendra là.
pub fn frame(context: &mut Context) -> Vec<u8> {
    let mut pixels = vec![0u8; WIDTH as usize * HEIGHT as usize * BYTES_PER_PIXEL];
    context
        .frame_end(&mut pixels, WIDTH)
        .expect("image rendue hors fenêtre");
    pixels
}

/// Un labyrinthe d'épreuve, par sa graine.
///
/// **Plusieurs graines plutôt qu'une** : le départ est le centre de la grille et
/// les volées sont tirées, donc la forme du sol sous les pieds du joueur change
/// d'une graine à l'autre — plat, en marches, ou en rampe. Une seule graine
/// n'éprouverait qu'une des trois, et sans dire laquelle.
pub fn maze(seed: u64) -> (Grid, World) {
    let grid = Grid::generate(Settings {
        extent: (16, 16, 2),
        seed,
        stairs: 6,
        ramps: 2,
        loops: 8,
    });
    let map = World::load(&export::world(&grid)).expect("carte engendrée valide");
    (grid, map)
}

/// Les graines éprouvées, dont celle du jeu.
pub const SEEDS: [u64; 6] = [0x5EED_1A8E, 1, 2, 3, 0xD1CE, 0xFACE];

/// Le pas de temps d'une image, à soixante par seconde.
///
/// **Ce qu'il sert à éprouver est la chute, et rien d'autre ne doit en dépendre** :
/// un prédicat de déplacement qui changerait de réponse selon la cadence serait un
/// défaut, et c'est pourquoi les épreuves de glissade l'emploient aussi — elles
/// passent sous une pesanteur qui les appuie au sol.
pub const DT: f32 = 1.0 / 60.0;

/// Les quatre côtés horizontaux, ceux qu'un pas de marche emprunte.
pub fn sides() -> Vec<Side> {
    Side::ALL.into_iter().filter(|s| !s.is_vertical()).collect()
}

/// Toutes les cases d'une grille, dans l'ordre des axes.
pub fn cases(grid: &Grid) -> Vec<(u32, u32, u32)> {
    let (width, height, levels) = grid.extent();
    let mut out = Vec::with_capacity((width * height * levels) as usize);
    for z in 0..levels {
        for y in 0..height {
            for x in 0..width {
                out.push((x, y, z));
            }
        }
    }
    out
}

/// La direction unitaire d'un côté, en unités de monde.
pub fn unit(side: Side) -> Vec3 {
    let (dx, dy, dz) = side.step();
    Vec3::new(dx as f32, dy as f32, dz as f32)
}

/// Les deux côtés horizontaux perpendiculaires à celui-ci.
///
/// Ce sont les tangentes d'un mur : les directions dans lesquelles une glissade
/// peut emporter ce qui reste du pas.
pub fn across(side: Side) -> Vec<Side> {
    sides()
        .into_iter()
        .filter(|s| *s != side && *s != side.facing())
        .collect()
}

/// Les vingt-six directions d'un pas d'épreuve : les axes, et toutes leurs
/// combinaisons.
///
/// **Les diagonales comptent autant que les axes**, et plus encore : c'est un pas
/// oblique qui distingue une glissade d'un arrêt, et c'est lui qui présente deux
/// normales à la fois dans un coin.
///
/// **Le vertical en fait partie, et la pesanteur l'a rendu quotidien** : chaque
/// image ajoute une composante de chute au pas demandé, donc un pas qui rencontre un
/// sol ou un plafond **en même temps** qu'un mur est le cas courant et non un cas
/// limite. C'est là que trois plans se présentent dans le même pas.
pub fn bearings() -> Vec<Vec3> {
    let mut out = Vec::with_capacity(26);
    for dx in [-1.0, 0.0, 1.0] {
        for dy in [-1.0, 0.0, 1.0] {
            for dz in [-1.0, 0.0, 1.0] {
                if dx != 0.0 || dy != 0.0 || dz != 0.0 {
                    out.push(Vec3::new(dx, dy, dz));
                }
            }
        }
    }
    out
}

/// Vrai si cette case a un sol plat, donc si aucune cage ne l'occupe.
///
/// **Comparer la cellule à l'identifiant de la case ne suffit pas** : pour le
/// **pied** d'une cage, les deux sont égaux — c'est elle qui donne son rang à la
/// cellule —, et seule la tête en diffère. Il faut donc interroger les volées.
///
/// C'est ce que veulent les deux prédicats de pas : le sol d'une cage monte, donc
/// un pas horizontal y rencontre une marche ou une rampe, ce qui est juste et
/// n'éprouve ni un mur ni un passage.
pub fn plain(grid: &Grid, at: (u32, u32, u32)) -> bool {
    !grid
        .stairs()
        .iter()
        .any(|stair| stair.foot == at || stair.head() == at)
}

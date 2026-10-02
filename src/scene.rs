// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! L'état du monde, et la scène qu'on en soumet au moteur.
//!
//! **`submit` ne lit ni horloge, ni entrée, ni tampon de sortie**, et c'est sa
//! raison d'être : la boucle de `screengine-play` ouvre une fenêtre, donc elle ne
//! servira pas à l'image animée du `README`, qui appellera le moteur directement.
//! Les deux chemins doivent construire la même scène, ce qui n'est possible que
//! si la construction vit hors du rappel. Écrite dedans, elle serait à défaire le
//! jour venu.
//!
//! Elle ne mute rien non plus : une scène se soumet deux fois sur la même pose
//! sans que rien ne bouge entre les deux, ce qu'un rendu en plusieurs passes fait
//! par construction.

use std::sync::Arc;

use screengine_play::screengine::{Context, Visibility};
use screengine_play::{Affine3, Camera, Error, Texture, Vec3, World, load_png};

use crate::maze::export::{self, CELL, LEVEL};
use crate::maze::grid::{Grid, Settings};

#[cfg(test)]
mod tests;

/// La hauteur de l'œil au-dessus du sol.
///
/// Elle vit ici le temps que `player.rs` existe : c'est une pose, pas une cote de
/// la carte, et l'étape 2 la déduira du centre d'un corps plutôt que du sol.
pub const EYE: f32 = 1.4;

/// La planche des murs.
///
/// **Provisoire, comme celle du sol.** Les deux servent à voir le décor, pas à
/// l'habiller : un labyrinthe que des démons parcourent demande plus sombre que
/// de la pierre propre. L'habillage se choisit à l'étape 8, avec le reste de
/// l'ambiance, et `assets/textures/` porte déjà deux variantes moussues.
const WALL: &[u8] = include_bytes!("../assets/textures/mur-42.png");

/// Celle du sol et du plafond, provisoire pour la même raison.
const FLOOR: &[u8] = include_bytes!("../assets/textures/sol-pave.png");

/// L'état du monde : la carte chargée, et ce qu'il faut pour la dessiner.
///
/// **Rien de la partie n'entre ici** — ni vie, ni score, ni pose de caméra. Le
/// monde est rechargeable à chaud, la partie est jetée au rechargement, et c'est
/// cette séparation qui rend l'édition possible.
pub struct Scenery {
    /// Le labyrinthe engendré, que le plan de contrôle relit.
    pub maze: Grid,
    /// La carte que le moteur en a tirée.
    pub map: World,
    /// Une texture par emplacement de matériau, dans l'ordre de la carte.
    materials: Vec<Arc<Texture>>,
}

impl Scenery {
    /// Engendre un labyrinthe, l'écrit en carte, et charge de quoi l'habiller.
    pub fn new(settings: Settings) -> Result<Self, Error> {
        let maze = Grid::generate(settings);
        let map = World::load(&export::world(&maze))?;

        let wall = Arc::new(load_png(WALL).expect("planche de mur du dépôt valide"));
        let floor = Arc::new(load_png(FLOOR).expect("planche de sol du dépôt valide"));
        // Le fichier porte des noms, jamais des images : l'hôte les découvre et
        // passe ses propres handles, dans l'ordre des emplacements.
        let materials = (0..map.material_count())
            .map(|rank| match map.material_name(rank) {
                Some("wall") => Arc::clone(&wall),
                _ => Arc::clone(&floor),
            })
            .collect();

        Ok(Self {
            maze,
            map,
            materials,
        })
    }

    /// Où l'on entre dans le labyrinthe, œil compris.
    pub fn entrance(&self) -> Vec3 {
        let (x, y, level) = self.maze.start();
        Vec3::new(
            (x as f32 + 0.5) * CELL,
            (y as f32 + 0.5) * CELL,
            level as f32 * LEVEL + EYE,
        )
    }
}

/// Ce qu'une image regarde.
///
/// La cellule accompagne la pose parce que le moteur ne retient ni l'une ni
/// l'autre : il ne connaît pas le monde entre deux appels, et c'est l'hôte qui
/// suit sa caméra.
pub struct View {
    /// La pose de la caméra.
    pub camera: Camera,
    /// La cellule qui la contient, ou zéro pour « nulle part ».
    pub cell: u32,
}

/// Soumet au moteur ce que cette vue montre du monde.
///
/// Rend ce que la traversée a pu déplier : `Incomplete` dit qu'elle a atteint une
/// de ses deux bornes et que l'image s'arrête une cellule plus loin, `NoCell` que
/// la pose est hors de tout volume. Aucun des deux n'est une erreur, et un
/// labyrinthe est le premier décor qui puisse les approcher — c'est `E1.5` qui
/// les relèvera.
pub fn submit(
    context: &mut Context,
    scenery: &Scenery,
    view: &View,
) -> Result<Visibility, screengine_play::screengine::Error> {
    context.set_camera(view.camera)?;
    context.submit_world_visible(
        Affine3::IDENTITY,
        &scenery.map,
        view.cell,
        // Aucune lightmap : la soumission retombe alors sur le chemin non
        // éclairé, sans erreur. L'ambiance est l'étape 8, et on ne règle pas un
        // tamisage dans un couloir qu'on vient d'ouvrir.
        None,
        |rank| scenery.materials.get(rank as usize),
    )
}

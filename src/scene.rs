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

use screengine_play::{
    Affine3, Camera, Context, Error, Texture, Vec3, Visibility, World, load_png,
};

use crate::maze::export;
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

/// Celle du sol, du plafond et des marches, provisoire pour la même raison.
///
/// **Les marches la prennent faute d'une planche à elles** : un escalier est
/// d'une seule matière, et c'est celle qu'on foule. Un matériau `stair` se
/// justifiera à l'étape 8, quand il y aura une image à lui donner.
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
    ///
    /// **Le centre de la case ne suffit pas** : si le départ tombe sur une case
    /// d'escalier, son centre est dans le solide sous les marches, et la caméra
    /// commence dans un mur. Rien ne l'interdit — le départ est le centre de la
    /// grille, et les volées sont tirées au hasard.
    pub fn entrance(&self) -> Vec3 {
        let spot = export::ground(&self.maze, self.maze.start());
        Vec3::new(spot[0], spot[1], spot[2] + EYE)
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
/// labyrinthe est le premier décor qui puisse les approcher, et relever ce que
/// cette valeur dit reste à faire.
///
/// L'erreur est celle du **noyau**, et son chemin reste long là où `Context` et
/// `Visibility` sont à plat : la boucle a sa propre `Error`, et les deux ne
/// peuvent pas porter le même nom dans la même surface.
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
        // éclairé, sans erreur. La carte porte pourtant ses lumières — c'est la
        // géométrie qui décide de ce qu'une lampe atteint, donc les écrire tard
        // obligerait à réexporter —, mais rien n'est cuit avant l'étape 8, et on
        // ne règle pas un tamisage dans un couloir qu'on vient d'ouvrir.
        None,
        |rank| scenery.materials.get(rank as usize),
    )
}

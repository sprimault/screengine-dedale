// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le joueur : un gabarit, une hauteur d'œil, et un corps qui porte le reste.
//!
//! **Ce qui se déplace est le centre d'un corps, jamais l'œil.** Un volume centré
//! sur l'œil flotterait au-dessus de ce qui est bas et le traverserait — c'est
//! géométriquement juste et parfaitement faux —, et l'œil se déduit donc du
//! corps par un décalage. Jamais l'inverse.
//!
//! **La politique de collision n'est pas ici, et c'est délibéré** : glissade,
//! chute, collage et franchissement ne parlent que d'une boîte et d'un pas, donc
//! elles vivent dans [`crate::body`], où une créature d'un autre gabarit les
//! emprunte. Ce qui reste ici est ce qui ne vaut que pour le joueur — les deux
//! cotes, ce qu'elles doivent au décor, et la naissance à l'entrée du labyrinthe.
//!
//! Ce qui n'y est pas encore, et qui se juge à l'œil : le pas de côté, et les
//! réglages de la marche.

use screengine_play::{Vec3, World};

use crate::body::Body;
use crate::maze::export;
use crate::maze::grid::Grid;

#[cfg(test)]
mod tests;

/// Les demi-étendues du corps, en unités de monde.
///
/// Soixante centimètres de côté sur un mètre quatre-vingt : de quoi passer dans
/// une cellule de trois mètres et sous un plafond de deux et demi, en gardant
/// l'empreinte qui dépasse un palier de cage — ce que `SLOPE` existe pour
/// corriger.
///
/// **C'est le gabarit que les épreuves de l'export emploient déjà**, et c'est la
/// raison de le garder : leur domaine de validité est celui-là, et un autre
/// gabarit sortirait de ce qui a été éprouvé sans que rien ne le dise.
pub const HALF: Vec3 = Vec3::new(0.3, 0.3, 0.9);

/// De combien l'œil est au-dessus du centre du corps.
///
/// Les pieds posés, l'œil se trouve donc à `HALF.z + EYE_ABOVE` du sol, soit un
/// mètre cinquante. Ce n'est pas une cote de la carte : elle décide de l'échelle
/// qu'on prête au décor, et elle s'est jugée à l'écran, cote lue dans le titre de
/// la fenêtre.
///
/// **Elle dépend du plafond et de ce qu'on tient en main, pas de l'anatomie.** Sous
/// un plafond de deux mètres cinquante, un mètre soixante paraissait trop haut — il
/// ne restait que quatre-vingt-dix centimètres au-dessus de l'œil. Sous trois
/// mètres vingt-cinq la même cote passait, et c'est l'arme en main qui a donné
/// l'échelle manquante : dix centimètres plus bas, le décor prend sa taille.
///
/// **Ni elle ni `HALF.z` ne sont exactes en binaire, et cela ne nuit pas** : elles
/// composent une pose de caméra, jamais une cote écrite dans la carte. L'exactitude
/// s'impose à ce qui s'apparie au bit près et à ce qui entre dans un volume signé,
/// pas à ce qui sert à regarder.
pub const EYE_ABOVE: f32 = 0.6;

// **Ce que le décor laisse de place, vérifié à la compilation.** Un corps plus
// large qu'une cellule coincerait dans un couloir, un corps plus haut que le
// plafond ne passerait nulle part, et l'œil sous le plafond est ce qui empêche de
// regarder à travers. Ce sont des relations entre constantes : elles n'ont rien à
// faire dans une épreuve, qui ne les vérifierait qu'après la compilation.
const _: () = assert!(2.0 * HALF.x < export::INNER);
const _: () = assert!(2.0 * HALF.y < export::INNER);
const _: () = assert!(2.0 * HALF.z < export::CEILING);
const _: () = assert!(HALF.z + EYE_ABOVE < export::CEILING);

/// Le joueur : un corps au gabarit du joueur, et l'œil qui s'en déduit.
pub struct Player {
    /// Le corps, qui porte la pose, la cellule et toute la politique.
    body: Body,
}

impl Player {
    /// Pose le joueur à l'entrée du labyrinthe, debout sur son sol.
    pub fn spawn(grid: &Grid, map: &World) -> Self {
        Self::stand(grid, map, grid.start())
    }

    /// Pose le joueur debout sur une case donnée.
    ///
    /// **Le départ n'est qu'un cas particulier**, et c'est ce qui rend la pose
    /// éprouvable ailleurs qu'à l'entrée : un prédicat de déplacement a besoin de
    /// se placer où il veut, et recopier ce calcul dans les épreuves le ferait
    /// diverger de celui du jeu.
    pub fn stand(grid: &Grid, map: &World, at: (u32, u32, u32)) -> Self {
        Self {
            body: Body::stand(HALF, grid, map, at),
        }
    }

    /// Déplace le joueur de ce qu'il peut parcourir, et rend la distance franchie.
    ///
    /// **Elle vaut zéro contre un mur**, et c'est ce dont le balancement de l'arme
    /// se sert : sa phase avance avec la distance réellement parcourue, donc des
    /// mains qui balancent alors qu'on est bloqué dénoncent un déplacement appliqué
    /// avant la collision.
    pub fn advance(&mut self, map: &World, moved: Vec3, dt: f32) -> f32 {
        self.body.advance(map, moved, dt)
    }

    /// Où l'œil se trouve, pose de la caméra.
    pub fn eye(&self) -> Vec3 {
        let centre = self.body.centre();
        Vec3::new(centre.x, centre.y, centre.z + EYE_ABOVE)
    }

    /// La cellule qui contient le corps, ou zéro.
    pub fn cell(&self) -> u32 {
        self.body.cell()
    }
}

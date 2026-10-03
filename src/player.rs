// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le corps du joueur : son volume, sa pose, et la cellule qui le contient.
//!
//! **Ce qui se déplace est le centre d'un corps, jamais l'œil.** Un volume centré
//! sur l'œil flotterait au-dessus de ce qui est bas et le traverserait — c'est
//! géométriquement juste et parfaitement faux —, et l'œil se déduit donc du
//! corps par un décalage. Jamais l'inverse.
//!
//! Le déplacement n'est pas encore filtré par le balayage : le vol libre décide
//! toujours de ce qui est parcouru, et ce module ne fait que porter la pose du
//! bon côté de la frontière. Ce qui change avec le filtrage est **l'entrée** de
//! [`Player::advance`], pas le reste.

use screengine_play::{Vec3, World};

use crate::maze::export::{self, SLOPE};
use crate::maze::grid::Grid;

#[cfg(test)]
mod tests;

/// Les demi-étendues du corps, en unités de monde.
///
/// Soixante centimètres de côté sur un mètre quatre-vingt : de quoi passer dans
/// une cellule de trois mètres et sous un plafond de deux et demi, en gardant
/// l'empreinte qui dépasse un palier de cage — ce que [`SLOPE`] existe pour
/// corriger.
///
/// **C'est le gabarit que les épreuves de l'export emploient déjà**, et c'est la
/// raison de le garder : leur domaine de validité est celui-là, et un autre
/// gabarit sortirait de ce qui a été éprouvé sans que rien ne le dise.
pub const HALF: Vec3 = Vec3::new(0.3, 0.3, 0.9);

/// De combien l'œil est au-dessus du centre du corps.
///
/// Les pieds posés, l'œil se trouve donc à `HALF.z + EYE_ABOVE` du sol, soit un
/// mètre quarante. Ce n'est pas une cote de la carte : elle décide de l'échelle
/// qu'on prête au décor, et elle s'est jugée à l'écran.
///
/// **Sept dixièmes, essayés d'abord, donnaient l'impression de toucher le
/// plafond** : il ne restait qu'un mètre dix au-dessus de l'œil sous deux mètres
/// cinquante, et le haut du corps passait à sept dixièmes de la dalle. Le compte
/// anatomique dit la même chose — l'œil à quatre-vingt-neuf centièmes d'un corps
/// d'un mètre quatre-vingts tombe au sommet du crâne, là où quatre-vingts
/// centièmes sont la bonne proportion.
pub const EYE_ABOVE: f32 = 0.5;

// **Ce que le décor laisse de place, vérifié à la compilation.** Un corps plus
// large qu'une cellule coincerait dans un couloir, un corps plus haut que le
// plafond ne passerait nulle part, et l'œil sous le plafond est ce qui empêche de
// regarder à travers. Ce sont des relations entre constantes : elles n'ont rien à
// faire dans une épreuve, qui ne les vérifierait qu'après la compilation.
const _: () = assert!(2.0 * HALF.x < export::INNER);
const _: () = assert!(2.0 * HALF.y < export::INNER);
const _: () = assert!(2.0 * HALF.z < export::CEILING);
const _: () = assert!(HALF.z + EYE_ABOVE < export::CEILING);

/// Le joueur : un volume, une pose, et la cellule où il se trouve.
pub struct Player {
    /// Le centre du corps.
    centre: Vec3,
    /// La cellule qui le contient, ou zéro s'il est hors du décor.
    cell: u32,
    /// Où son centre était au pas précédent, ce dont le suivi a besoin.
    previous: Vec3,
}

impl Player {
    /// Pose le joueur à l'entrée du labyrinthe, debout sur son sol.
    ///
    /// **La cellule vient de l'export et non d'une localisation** : `cover` la
    /// rend par la seule position de la case, là où `locate` parcourt toutes les
    /// cellules et toutes leurs faces pour retrouver ce que l'export sait déjà.
    ///
    /// **La pose se résout par un balayage, elle ne se calcule pas.** Le corps part
    /// assez haut pour qu'aucune pente du décor ne le pénètre et descend jusque
    /// **sous** la cote du sol : le balayage rencontre donc toujours quelque chose,
    /// et la fraction rendue pose le corps juste au-dessus, quelle que soit la
    /// forme du sol — plat, en marches ou en rampe. Calculer cette cote
    /// demanderait de savoir laquelle des trois, et c'est ce que le jeu n'a pas à
    /// connaître.
    ///
    /// **Viser sous le sol plutôt qu'au ras** n'est pas une précaution : une cible
    /// posée un quart de millimètre au-dessus tombe dans la bande de contact du
    /// moteur, le balayage n'y rencontre rien, et le corps resterait là où le
    /// calcul l'a mis — c'est-à-dire là où on voulait précisément ne pas décider.
    pub fn spawn(grid: &Grid, map: &World) -> Self {
        let at = grid.start();
        let cell = export::cover(grid, at);
        let spot = export::ground(grid, at);

        // Le dégagement qu'une pente impose : le coin aval du bas monte autant
        // que le corps est profond, fois la pente.
        let clear = HALF.x * SLOPE;
        let above = Vec3::new(spot[0], spot[1], spot[2] + HALF.z + clear);
        let below = Vec3::new(above.x, above.y, spot[2] + HALF.z - clear);

        let centre = match map.sweep(cell, HALF, above, below) {
            Some(hit) if !hit.start_solid => above + (below - above) * hit.fraction,
            // Un départ que le décor ne devrait pas offrir : on garde la cote
            // haute, la moins pénétrante des deux, et l'épreuve de pose dit que
            // le cas est arrivé.
            _ => above,
        };

        Self {
            centre,
            cell,
            previous: centre,
        }
    }

    /// Déplace le corps de ce qui a été parcouru, et suit sa cellule.
    ///
    /// **Le déplacement arrive filtré ou non, et ce module n'en sait rien** :
    /// c'est ce qui permettra au balayage de s'interposer sans toucher à la suite.
    ///
    /// Le suivi porte sur le **centre du corps** et non sur l'œil, parce que c'est
    /// lui qui a un volume et que c'est son volume qu'on balaie. Le moteur ne se
    /// relocalise jamais de lui-même : zéro veut dire « sorti du décor », et tant
    /// que le vol libre traverse les murs il faut le lui redemander.
    pub fn advance(&mut self, map: &World, moved: Vec3) {
        self.centre = self.centre + moved;
        let found = map.track(self.cell, self.previous, self.centre);
        self.cell = if found == 0 {
            map.locate(self.centre)
        } else {
            found
        };
        self.previous = self.centre;
    }

    /// Où l'œil se trouve, pose de la caméra.
    pub fn eye(&self) -> Vec3 {
        Vec3::new(self.centre.x, self.centre.y, self.centre.z + EYE_ABOVE)
    }

    /// La cellule qui contient le corps, ou zéro.
    pub fn cell(&self) -> u32 {
        self.cell
    }
}

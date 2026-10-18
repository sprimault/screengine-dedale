// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le réticule : quatre traits autour du centre de la vue, et un vide au milieu.
//!
//! **Il marque l'endroit d'où le rayon part**, et c'est tout son objet. Sans lui, le
//! seul repère de visée est le canon de l'arme, qui est dessiné décalé — à quelque
//! quarante-cinq pixels à droite et quatre-vingt-dix pixels sous le centre —, donc qui
//! désigne un autre point que celui qu'on touche. Recentrer l'arme masquerait la cible,
//! et son cadrage s'est réglé à l'écran : c'est au réticule de dire où l'on vise.
//!
//! **Un vide au milieu plutôt qu'une croix pleine** : ce qu'on vise est petit — un
//! démon à vingt mètres fait quelques pixels —, et un trait qui passe dessus le cache
//! au moment précis où on le regarde.
//!
//! **Et un liseré sombre sous le blanc**, parce que le décor n'a pas de teinte fixe :
//! un réticule blanc seul disparaît sur une brique claire, un réticule noir seul
//! disparaît dans un couloir sombre. Deux passes coûtent quelques dizaines de pixels et
//! se lisent partout.

use screengine_play::Output;

use super::{block, scale};

#[cfg(test)]
mod tests;

/// Le vide entre le centre et le début d'un trait, en pixels à la hauteur d'ouverture.
const GAP: u32 = 3;

/// La longueur d'un trait, en pixels à la hauteur d'ouverture.
const ARM: u32 = 4;

/// Le blanc du trait.
const INK: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

/// Le liseré posé sous le trait, qui le détache d'un fond clair.
const EDGE: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

/// Le côté minimum d'un tampon qui puisse porter le réticule entier, en pixels.
///
/// Du centre : le vide, le bras, et le liseré qui déborde — deux fois, plus la colonne
/// du centre elle-même. Tout au facteur, puisque tout y est dessiné.
fn span(unit: u32) -> u32 {
    (2 * (GAP + ARM + 1) + 1) * unit
}

/// Dessine le réticule au centre du tampon de sortie.
///
/// **Le centre se prend du tampon à chaque image, jamais d'une constante.** La
/// résolution interne du moteur n'est pas celle de la fenêtre et peut changer en cours
/// de jeu ; une cote d'écran figée serait juste le jour où on l'écrit et fausse au
/// premier changement, sans que rien ne le dise.
///
/// **Et les longueurs suivent, depuis que la résolution interne suit la fenêtre.**
/// C'était l'inverse tant qu'elle était figée : le jeu rendait bas et remontait l'image
/// en entier, donc un trait de quatre pixels gardait son grain quelle que soit la
/// fenêtre. Maintenant que l'image grandit, un trait figé se perd au centre d'un écran
/// large — ce que le module annonçait, et c'est le facteur de l'interface qui le règle.
pub fn draw(output: &mut Output<'_>) {
    let (width, height) = (output.width(), output.height());
    let unit = scale(height);
    // **Un tampon trop petit ne reçoit rien**, et ce n'est pas de la défensive : les
    // cotes viennent de l'hôte, le bras et son liseré se soustraient du centre, et un
    // entier non signé ne descend pas sous zéro — il déborde par le haut, donc la
    // panique arriverait loin de sa cause.
    let reach = span(unit);
    if width < reach || height < reach {
        return;
    }
    let (cx, cy) = (width / 2, height / 2);
    let (gap, arm) = (GAP * unit, ARM * unit);

    // Chaque trait : son liseré d'abord, débordant d'un facteur de tous côtés, puis le
    // blanc par-dessus. L'ordre est ce qui détache le trait, et il n'est pas négociable.
    for (x, y, long, across) in [
        // À gauche et à droite du centre, puis au-dessus et en dessous.
        (cx - gap - arm, cy, arm, true),
        (cx + gap + unit, cy, arm, true),
        (cx, cy - gap - arm, arm, false),
        (cx, cy + gap + unit, arm, false),
    ] {
        let (w, h) = match across {
            true => (long, unit),
            false => (unit, long),
        };
        block(output, x - unit, y - unit, w + 2 * unit, h + 2 * unit, EDGE);
        block(output, x, y, w, h, INK);
    }
}

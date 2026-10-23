// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Ce qui reste à tirer : une cartouche dans un carré, et le compte sous elle.
//!
//! **Les deux choses qui s'épuisent sur la même ligne**, et c'est ce qui décide de sa
//! place : la vie et les munitions se surveillent au même instant — celui où une
//! créature arrive —, donc les séparer aux deux coins de l'écran coûterait un
//! aller-retour du regard à chaque fois. Le compteur de score, lui, ne se lit jamais
//! dans l'urgence et garde son coin.
//!
//! **Et le haut de l'écran reste libre**, ce qui n'est pas un détail : le plan de
//! l'étape 9 le prendra en entier, et c'est le seul élément d'interface qui ne tiendra
//! pas dans un coin.
//!
//! **La cartouche est dessinée en blocs, et non chargée d'une planche.** Ce n'est pas un
//! pis-aller : une vignette détourée pour un objet posé au sol est riche en détails qui
//! deviennent une bouillie à vingt pixels, là où trois rectangles étagés donnent une
//! silhouette franche à toutes les échelles. C'est déjà ainsi que le réticule se dessine,
//! et que la marque et l'éclat sont produits.
//!
//! **Il s'ancre sur la jauge et non sur le tampon**, par [`gauge::right`] : la longueur
//! de la jauge est une cote de la jauge, et la recopier ici ferait deux valeurs qui
//! divergent au premier réglage.

use screengine_play::Output;

use super::glyph::{self, Glyphs};
use super::{EDGE, INSET, PLATE, block, gauge, scale};

#[cfg(test)]
mod tests;

/// L'échelle des chiffres, à la hauteur d'ouverture.
///
/// **Un, et non deux comme le compteur de score** : le carré se dimensionne sur son
/// contenu, donc trois rangs à l'échelle du score lui donnaient quarante-six pixels de
/// côté — vu à l'écran, un bloc énorme contre une jauge qui en fait sept d'épaisseur. À
/// l'échelle un, le même contenu tient dans vingt-cinq.
///
/// **Ce que ça coûte est la symétrie avec le score**, et c'est assumé : les deux nombres
/// ne se lisent plus à la même taille, mais ils ne se lisent pas dans le même moment non
/// plus — le score se consulte, les munitions se surveillent, et un carré qui mange le
/// bas de l'écran se surveille moins bien qu'il n'encombre.
const SIZE: u32 = 1;

/// L'écart entre le liseré de la jauge et celui du carré, en pixels à la hauteur
/// d'ouverture.
///
/// **Six, et ce qu'il doit faire est séparer sans détacher** : collés, les deux liserés
/// n'en font qu'un et les deux plaques se lisent comme un seul élément ; trop écartés, le
/// carré paraît appartenir au milieu de l'écran plutôt qu'à la jauge.
const APART: u32 = 6;

/// Ce que la plaque laisse d'air autour de son contenu, en pixels à la hauteur
/// d'ouverture.
const PAD: u32 = 2;

/// La largeur de la cartouche, en pixels à la hauteur d'ouverture.
///
/// **Quatre, et c'est un nombre pair par construction** : l'ogive s'étage en retirant une
/// unité d'interface de chaque côté, donc une largeur impaire donnerait une pointe
/// décentrée d'un demi-pixel — ce qui se voit, une silhouette de quatre pixels n'ayant
/// pas de quoi absorber l'erreur.
const CASING: u32 = 4;

/// Sa hauteur, en pixels à la hauteur d'ouverture.
///
/// Sept, pour une largeur de quatre : c'est la proportion qui la fait lire comme une
/// cartouche et non comme un jeton. Le premier pixel est l'ogive, les six autres le
/// corps.
const TALL: u32 = 7;

/// L'écart entre la cartouche et le nombre, en pixels à la hauteur d'ouverture.
const BREATH: u32 = 2;

/// Le gabarit sur lequel le côté du carré se dimensionne.
///
/// **Trois rangs, et au-delà la plaque s'étend vers la droite** plutôt que de tronquer.
/// Le cas n'arrive pas avec la dotation d'aujourd'hui, et c'est le bon échec s'il arrive
/// — un chiffre tronqué se lirait comme un autre nombre.
const WIDEST: &str = "000";

/// Dessine la cartouche et le compte, à droite de la jauge.
///
/// **Un tampon trop petit ne reçoit rien**, comme pour la jauge et pour la même raison :
/// le coin bas se calcule par soustraction, donc sur un entier non signé une hauteur plus
/// faible que la marge déborde par le haut au lieu de descendre sous zéro. En largeur, ce
/// qu'on évite est une plaque amputée, qui se lirait comme un compteur vide — soit
/// l'inverse de ce qu'il dit.
pub fn draw(output: &mut Output<'_>, glyphs: &Glyphs, ammo: u32) {
    let unit = scale(output.height());
    let (size, pad, inset) = (SIZE * unit, PAD * unit, INSET * unit);

    let text = format!("{ammo}");
    // **Le côté vient de la plus grande des deux dimensions du contenu**, et non de sa
    // largeur seule : à l'échelle un, la cartouche et sa ligne de chiffres sont plus
    // hautes que trois rangs ne sont larges, donc un côté pris sur la largeur laisserait
    // le contenu déborder de sa propre plaque.
    let content = (
        glyphs.width(&text, size).max(glyphs.width(WIDEST, size)),
        TALL * unit + BREATH * unit + glyphs.height(size),
    );
    let side = content.0.max(content.1) + 2 * pad;

    let left = gauge::right(output.height()) + APART * unit;
    if output.width() < left + side + unit || output.height() < side + inset + unit {
        return;
    }
    let top = output.height() - inset - side;

    let edge = 2 * unit;
    block(
        output,
        left - unit,
        top - unit,
        side + edge,
        side + edge,
        EDGE,
    );
    block(output, left, top, side, side, PLATE);

    // La cartouche et le nombre forment un bloc, centré dans le carré : c'est l'ensemble
    // qui se centre et non chacun des deux, sans quoi l'écart entre eux varierait avec le
    // compte.
    let first = top + (side - content.1) / 2;

    casing(output, left + (side - CASING * unit) / 2, first, unit);

    let ink = left + (side - glyphs.width(&text, size)) / 2;
    let under = first + TALL * unit + BREATH * unit;
    glyphs.draw(output, (ink, under), &text, size, glyph::INK);
}

/// Dessine la cartouche, son ogive en haut.
///
/// **Deux rectangles, et l'ogive en un seul palier** : à quatre pixels de large, un
/// second palier vaudrait zéro pixel — c'est ce que la première écriture faisait, sur une
/// cartouche deux fois plus grande. Ce qui reste est le minimum qui distingue une
/// cartouche d'un bâton : un sommet plus étroit que le corps.
///
/// **Le palier se retire d'une unité d'interface de chaque côté**, et non d'un pixel : à
/// facteur deux, une ogive étagée pixel par pixel sur une silhouette deux fois plus
/// grande redeviendrait un bâton. C'est le même raisonnement que la diagonale du plan de
/// contrôle.
fn casing(output: &mut Output<'_>, x: u32, y: u32, unit: u32) {
    let wide = CASING * unit;

    block(output, x + unit, y, wide - 2 * unit, unit, glyph::INK);
    block(output, x, y + unit, wide, TALL * unit - unit, glyph::INK);
}

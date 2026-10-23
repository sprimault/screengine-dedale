// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le sac : un casier par sorte, son pictogramme et son compte sous lui.
//!
//! **Le contenu se voit en permanence, et c'est le parti pris** : un inventaire qui se
//! déplie cache ce qu'on a jusqu'à ce qu'on le demande, et ce jeu veut qu'on compte ses
//! munitions sans y penser. Deux casiers aujourd'hui, les trésors de l'étape 14 en
//! ajouteront.
//!
//! **Les choses qui s'épuisent sur la même ligne**, et c'est ce qui décide de sa place :
//! la vie, les munitions et les fioles se surveillent au même instant — celui où une
//! créature arrive —, donc les séparer aux coins de l'écran coûterait un aller-retour du
//! regard à chaque fois. Le compteur de score, lui, ne se lit jamais dans l'urgence et
//! garde son coin.
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

/// La teinte du rappel de touche.
///
/// **Plus terne que l'encre du compte, et c'est ce qui le met à sa place** : le rappel se
/// lit une fois, quand on cherche comment boire ; le compte se lit en jouant. À la même
/// encre, les deux se disputeraient le regard dans un carré de vingt-cinq pixels.
const HINT: [u8; 4] = [0x80, 0x7C, 0x74, 0xFF];

/// Le rappel de touche d'un casier, quand il en a un.
///
/// **Seule la fiole en porte**, parce qu'elle est seule à se consommer : on ne « boit » pas
/// une munition, elle part au tir. Les trésors de l'étape 14 en auront chacun un.
fn hint(sort: Sort) -> Option<&'static str> {
    match sort {
        Sort::Box => None,
        Sort::Vial => Some("F"),
    }
}

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
pub fn draw(output: &mut Output<'_>, glyphs: &Glyphs, ammo: u32, vials: u32) {
    for (rank, (count, sort)) in [(ammo, Sort::Box), (vials, Sort::Vial)]
        .into_iter()
        .enumerate()
    {
        slot(output, glyphs, rank as u32, count, sort);
    }
}

/// Ce qu'un casier montre.
///
/// **Deux pictogrammes et non deux planches** : à vingt pixels, ce qui distingue une
/// cartouche d'une fiole est une silhouette, et trois rectangles la donnent plus
/// franchement qu'une vignette réduite douze fois.
#[derive(Clone, Copy)]
enum Sort {
    /// Les munitions.
    Box,
    /// Les fioles de soin.
    Vial,
}

/// Le côté d'un casier, en pixels, à ce facteur.
///
/// **Le même pour tous les casiers, et pris sur le gabarit et non sur le compte** : des
/// casiers de largeurs différentes feraient une rangée en dents de scie, et un casier qui
/// s'élargit en passant la dizaine déplacerait tous ses voisins. Les plafonds de la course
/// — quatre-vingt-dix-neuf munitions, neuf fioles — garantissent que deux rangs suffisent,
/// donc le gabarit de trois n'est jamais dépassé.
///
/// **La plus grande des deux dimensions du contenu** : à l'échelle un, le pictogramme et
/// sa ligne de chiffres sont plus hauts que trois rangs ne sont larges, donc un côté pris
/// sur la largeur laisserait le contenu déborder de sa propre plaque.
fn side(glyphs: &Glyphs, unit: u32) -> u32 {
    let size = SIZE * unit;
    let content = glyphs
        .width(WIDEST, size)
        .max(TALL * unit + BREATH * unit + glyphs.height(size));

    content + 2 * PAD * unit
}

/// Le coin haut gauche du casier de ce rang, et le côté commun.
///
/// **Chaque casier s'ancre sur son voisin de gauche**, le premier sur la jauge : c'est ce
/// qui fait qu'un casier ajouté à l'étape 14 ne demande aucune reprise des autres.
/// **Rend `None` quand le casier ne tient pas**, et c'est ce qui fait que le dessin et le
/// clic s'accordent par construction : la condition est écrite une fois, donc un casier
/// qu'on ne dessine pas n'est jamais cliquable.
///
/// **La garde précède la soustraction, et pas par précaution** : sur un entier non signé,
/// une hauteur plus faible que la marge et le côté ne descend pas sous zéro — elle déborde
/// par le haut, et la panique arriverait loin de sa cause.
fn corner(width: u32, height: u32, glyphs: &Glyphs, rank: u32) -> Option<(u32, u32, u32)> {
    let unit = scale(height);
    let side = side(glyphs, unit);
    let apart = APART * unit;

    let left = gauge::right(height) + apart + rank * (side + apart);
    let fits = width >= left + side + unit && height >= side + INSET * unit + unit;
    fits.then(|| (left, height - INSET * unit - side, side))
}

/// Dessine un casier : sa plaque, son pictogramme, et son compte sous lui.
///
/// **Un tampon trop petit ne reçoit rien**, comme pour la jauge et pour la même raison :
/// le coin bas se calcule par soustraction, donc sur un entier non signé une hauteur plus
/// faible que la marge déborde par le haut au lieu de descendre sous zéro. En largeur, ce
/// qu'on évite est une plaque amputée, qui se lirait comme un compteur vide — soit
/// l'inverse de ce qu'elle dit.
fn slot(output: &mut Output<'_>, glyphs: &Glyphs, rank: u32, count: u32, sort: Sort) {
    let unit = scale(output.height());
    let size = SIZE * unit;
    let Some((left, top, side)) = corner(output.width(), output.height(), glyphs, rank) else {
        return;
    };

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

    // Le pictogramme et le nombre forment un bloc, centré dans le carré : c'est l'ensemble
    // qui se centre et non chacun des deux, sans quoi l'écart entre eux varierait avec le
    // compte.
    let tall = TALL * unit + BREATH * unit + glyphs.height(size);
    let first = top + (side - tall) / 2;

    let wide = CASING * unit;
    match sort {
        Sort::Box => casing(output, left + (side - wide) / 2, first, unit),
        Sort::Vial => flask(output, left + (side - wide) / 2, first, unit),
    }

    let text = format!("{count}");
    let ink = left + (side - glyphs.width(&text, size)) / 2;
    let under = first + TALL * unit + BREATH * unit;
    glyphs.draw(output, (ink, under), &text, size, glyph::INK);

    // **Le rappel au coin haut droit, par-dessus la plaque** : le pictogramme est centré et
    // ne fait que quatre pixels de large, donc ce coin est libre sans qu'il faille agrandir
    // le casier. À l'échelle du facteur seul et non à celle des chiffres — il doit se voir
    // sans se compter.
    if let Some(key) = hint(sort) {
        let corner = (
            left + side - PAD * unit - glyphs.width(key, unit),
            top + PAD * unit,
        );
        glyphs.draw(output, corner, key, unit, HINT);
    }
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

/// Dessine une fiole : un col étroit, puis une panse pleine largeur.
///
/// **L'inverse de la cartouche, et c'est ce qui les distingue** : celle-ci s'étage une
/// fois en haut sur un corps long, la fiole garde un col sur un tiers de sa hauteur avant
/// de s'élargir. À quatre pixels de large, c'est la seule différence de silhouette
/// disponible — et elle suffit, les deux ne se lisant jamais isolément mais côte à côte.
fn flask(output: &mut Output<'_>, x: u32, y: u32, unit: u32) {
    let wide = CASING * unit;
    let neck = TALL * unit / 3;

    block(output, x + unit, y, wide - 2 * unit, neck, glyph::INK);
    block(output, x, y + neck, wide, TALL * unit - neck, glyph::INK);
}

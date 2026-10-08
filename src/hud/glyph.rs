// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le texte à l'écran, lu case par case dans une planche de glyphes.
//!
//! **Un module frère de `sheet.rs` et non lui**, et la frontière est la même que celle
//! qui tient `map_view.rs` hors d'ici : le rectangle d'une planche de vues rend des
//! texels destinés à une **soumission**, un glyphe alimente le tampon d'**écran** après
//! la fin d'image. Trois des quatre déclarations publiques de `sheet.rs` sont en outre
//! vides de sens pour un caractère — secteur angulaire, phase de cycle, mouvement de
//! créature.
//!
//! **Seul l'alpha de la planche sert.** Elle se charge en texture masquée, le dessin
//! est en blanc opaque sur fond nul, et la couleur vient de l'appelant : un libellé
//! peut ainsi changer de teinte sans recuire quoi que ce soit, et le blanc ne sert
//! qu'à rendre la planche lisible quand on l'ouvre.
//!
//! **L'avance est une table, et c'est le prix du proportionnel.** La police est
//! proportionnelle — de quatre à neuf pixels selon le caractère —, et l'avance ne se
//! retrouve pas en lisant les alphas : l'espacement entre lettres est dessiné dans le
//! glyphe, si bien que la dernière colonne encrée s'arrête une à trois colonnes avant
//! l'avance. La table est donc une seconde source, et
//! `aucune_encre_ne_deborde_de_son_avance` la rattache à la planche dans le sens qui
//! fait mal — une avance trop courte fait chevaucher deux lettres, et l'épreuve rougit.
//! Une avance trop longue ne se mesure pas d'ici : elle n'écarte que l'espacement.

use screengine_play::{Output, Texture, load_png_masked};

use super::block;

#[cfg(test)]
mod tests;

/// La planche, intégrée au binaire.
const SHEET: &[u8] = include_bytes!("../../assets/fonts/glyphes-8x8-8.png");

/// Le premier caractère du répertoire, l'espace.
const FIRST: u32 = 32;

/// Le côté d'une case, en texels.
const CELL: u32 = 8;

/// Les cases par rangée de la planche.
const COLUMNS: u32 = 8;

/// Le décalage de l'octet d'alpha dans un texel.
///
/// Le moteur range ses texels en ARGB, et la planche étant masquée, cet octet vaut
/// tout ou rien : il est le pochoir, et le reste du texel ne sert pas ici.
const ALPHA: u32 = 24;

/// L'avance de chaque glyphe, en pixels, dans l'ordre du répertoire.
///
/// **Relevée dans la police, pas choisie** : la chaîne la tire de la table de
/// métriques du fichier à la taille où elle cuit la planche, et l'imprime pour être
/// recopiée ici. Quarante-quatre glyphes sur soixante-quatre avancent de sept, les
/// dix chiffres en font partie — un score ne tremble donc pas quand il change.
const ADVANCE: [u8; 64] = [
    4, 5, 7, 8, 7, 9, 7, 5, //
    7, 7, 7, 7, 5, 6, 5, 5, //
    7, 7, 7, 7, 7, 7, 7, 7, //
    7, 7, 5, 5, 5, 6, 5, 7, //
    9, 7, 7, 7, 7, 7, 7, 7, //
    7, 5, 7, 7, 7, 9, 7, 7, //
    7, 7, 7, 7, 7, 7, 7, 9, //
    7, 7, 7, 7, 5, 7, 7, 5, //
];

/// Le rang d'un caractère dans le répertoire, ou rien s'il n'y est pas.
///
/// **Les minuscules se replient sur les majuscules**, qui sont à trente-deux de
/// distance en ASCII : le répertoire s'arrête au souligné, et un libellé écrit en bas
/// de casse rendrait sinon une suite de trous. Ce qui reste dehors — un accent, un
/// caractère non latin — ne dessine rien **et n'avance pas**, si bien que le mot se
/// raccourcit : une faute qui se voit vaut mieux qu'un blanc qu'on prend pour une
/// espace.
fn rank(character: char) -> Option<usize> {
    let code = match character {
        'a'..='z' => character as u32 - 32,
        _ => character as u32,
    };
    (FIRST..FIRST + ADVANCE.len() as u32)
        .contains(&code)
        .then(|| (code - FIRST) as usize)
}

/// La planche chargée, prête à écrire.
///
/// **Ni du monde ni de la partie** : c'est une ressource, chargée une fois et jamais
/// modifiée. La ranger dans l'un des deux ferait passer un asset pour un état.
pub struct Glyphs {
    /// La planche décodée, en texture masquée.
    sheet: Texture,
}

impl Default for Glyphs {
    fn default() -> Self {
        Self::new()
    }
}

impl Glyphs {
    /// Décode la planche du binaire.
    ///
    /// L'`expect` est l'exception admise aux assets intégrés : ce qui échouerait est
    /// une planche du dépôt invalide, donc un défaut de livraison et non un cas
    /// d'exécution.
    pub fn new() -> Self {
        Self {
            sheet: load_png_masked(SHEET).expect("planche de glyphes du dépôt valide"),
        }
    }

    /// La largeur qu'un texte occupera, en pixels.
    ///
    /// Elle sert à centrer, et elle compte ce que [`Glyphs::draw`] avancera —
    /// caractères hors répertoire compris, c'est-à-dire pour rien.
    pub fn width(&self, text: &str, scale: u32) -> u32 {
        text.chars()
            .filter_map(rank)
            .map(|rank| ADVANCE[rank] as u32 * scale)
            .sum()
    }

    /// Écrit un texte, son coin haut gauche en `at`.
    ///
    /// **L'échelle réplique les pixels à facteur entier**, ce qu'une police pixel
    /// supporte sans perte — et c'est pourquoi la planche n'est cuite qu'à une taille.
    /// Ce qui sort du tampon est sauté par [`super::block`], donc un texte trop long
    /// se découpe au bord au lieu de déborder.
    ///
    /// **Le coin est un couple et non deux arguments**, là où [`super::block`] les
    /// sépare : avec le texte, l'échelle et la teinte, trois `u32` de suite se
    /// permutent sans que rien ne le dise.
    pub fn draw(
        &self,
        output: &mut Output<'_>,
        at: (u32, u32),
        text: &str,
        scale: u32,
        color: [u8; 4],
    ) {
        let (x, y) = at;
        let mut pen = x;
        for rank in text.chars().filter_map(rank) {
            let origin = (rank as u32 % COLUMNS * CELL, rank as u32 / COLUMNS * CELL);
            for row in 0..CELL {
                for column in 0..CELL {
                    if self.inked(origin, column, row) {
                        let (left, top) = (pen + column * scale, y + row * scale);
                        block(output, left, top, scale, scale, color);
                    }
                }
            }
            pen += ADVANCE[rank] as u32 * scale;
        }
    }

    /// Le texel d'une case est-il encré ?
    ///
    /// **La case entière est parcourue, avance comprise**, parce qu'il n'y a rien à y
    /// gagner : `aucune_encre_ne_deborde_de_son_avance` tient qu'aucun texel n'est
    /// allumé au-delà, donc borner la boucle sur l'avance retirerait des colonnes
    /// vides.
    fn inked(&self, origin: (u32, u32), column: u32, row: u32) -> bool {
        let (u, v) = ((origin.0 + column) as i32, (origin.1 + row) as i32);
        self.sheet.texel(0, u, v) >> ALPHA != 0
    }
}

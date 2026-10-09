// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le compteur de score, en bas à droite du tampon de sortie.
//!
//! **Les zéros de tête sont ce qui l'empêche de glisser.** Les dix chiffres de la
//! planche avancent de sept pixels, donc aucun ne bouge quand un autre change ; mais
//! un nombre cadré à droite se décale d'un cran entier au passage à l'ordre de
//! grandeur suivant, et un compteur qui saute attire l'œil au pire moment — celui
//! où un démon vient de tomber. À largeur fixe, le dessin est le même à chaque
//! image et seule l'encre change de place.
//!
//! **Le nombre seul, sans libellé.** Un `SCORE` devant coûterait cinq glyphes et une
//! espace pour dire ce qu'un nombre qui monte en tirant dit déjà, et il occuperait la
//! moitié d'un coin que la vue de dessus de l'étape 9 convoitera.
//!
//! **Symétrique de la jauge, et c'est le seul coin qui restait** : le plan de contrôle
//! tient le haut gauche, la jauge le bas gauche, le réticule le centre. Les deux
//! choses qu'on surveille en jouant encadrent ainsi le bas de l'écran.
//!
//! **Et il porte la plaque de la jauge, parce qu'une ombre n'a pas suffi.** Vu à
//! l'écran le 2026-10-10 : l'encre claire sur ombre se lit presque partout, et un peu
//! moins par endroits sur le sol pavé, qui est clair et détaillé. Un fond sombre et
//! son liseré rendent le chiffre lisible quel que soit ce qu'il y a derrière, et c'est
//! la réponse que la jauge et le plan ont déjà reçue pour la même raison. L'ombre
//! portée disparaît du même coup : sur une plaque, elle ne détacherait plus rien.

use screengine_play::Output;

use super::glyph::{self, Glyphs};
use super::{EDGE, PLATE, block};

/// L'échelle des chiffres.
///
/// Deux, soit seize pixels de haut : la jauge fait sept d'épaisseur et le compteur
/// doit se lire d'un coup d'œil sans dominer la vue. La cote se juge à l'écran.
const SCALE: u32 = 2;

/// La marge aux bords droit et bas, en pixels.
///
/// La même que celle de la jauge, pour que les deux reposent sur la même ligne.
const INSET: u32 = 8;

/// Le nombre de rangs du compteur, zéros de tête compris.
///
/// **Cinq, et c'est l'enchaînement de l'étape 7 qui le fixe** : une traversée ne
/// croise qu'une poignée de créatures, mais la course cumule d'un labyrinthe au
/// suivant. À cent par démon, cinq rangs tiennent neuf cent quatre-vingt-dix-neuf
/// créatures — et au-delà le nombre s'étend d'un cran vers la gauche plutôt que de
/// se tronquer, ce qui est le bon échec.
const DIGITS: usize = 5;

/// Ce que la plaque laisse d'air autour des chiffres, en pixels.
///
/// **Deux, soit un pixel de glyphe à l'échelle** : sans air, l'encre touche le liseré
/// et le chiffre se lit comme s'il était coupé. La police dessine déjà son espacement
/// latéral, donc l'air qui manque est celui du haut et du bas.
const PAD: u32 = 2;

#[cfg(test)]
mod tests;

/// Le compteur tel qu'il s'écrit, rangs de tête compris.
///
/// **À part du dessin pour être éprouvée** : c'est ici que se décide si le compteur
/// glisse, et une largeur constante se mesure sur la chaîne — la mesurer sur les
/// pixels supposerait de savoir où l'encre d'un chiffre commence dans sa case, ce que
/// la planche ne promet pas.
fn digits(score: u32) -> String {
    format!("{score:0DIGITS$}")
}

/// Dessine le compteur, cadré au coin bas droit.
///
/// **Un tampon trop court ne reçoit rien** : les deux coins se calculent par
/// soustraction, donc sur un entier non signé un tampon plus petit que la plaque ne
/// passe pas sous zéro — il déborde par le haut, et le compteur partirait à l'autre
/// bout de la ligne. Le pixel de plus qu'exige le refus est celui du liseré, qui se
/// dessine en dehors. C'est le même refus que celui de la jauge, pour la même raison.
pub fn draw(output: &mut Output<'_>, glyphs: &Glyphs, score: u32) {
    let text = digits(score);
    let plate = (
        glyphs.width(&text, SCALE) + 2 * PAD,
        glyphs.height(SCALE) + 2 * PAD,
    );
    if output.width() < INSET + plate.0 + 1 || output.height() < INSET + plate.1 + 1 {
        return;
    }

    let left = output.width() - INSET - plate.0;
    let top = output.height() - INSET - plate.1;

    block(output, left - 1, top - 1, plate.0 + 2, plate.1 + 2, EDGE);
    block(output, left, top, plate.0, plate.1, PLATE);
    glyphs.draw(output, (left + PAD, top + PAD), &text, SCALE, glyph::INK);
}

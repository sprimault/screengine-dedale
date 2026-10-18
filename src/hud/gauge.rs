// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! La jauge de vie : un fond, un liseré, et un remplissage qui dit ce qui reste.
//!
//! **L'ancrage et les longueurs se prennent tous deux du tampon, à chaque image.** La
//! résolution interne n'est pas celle de la fenêtre et change en cours de partie, donc
//! le coin se recalcule — et les cotes avec lui, depuis que la résolution suit la
//! fenêtre : une épaisseur figée en pixels maigrirait d'autant que l'image grandit, et
//! sept pixels sur une image trois fois plus haute se lisent trois fois plus fins. Les
//! constantes ci-dessous sont donc les cotes **à la hauteur d'ouverture**, que le
//! facteur de l'interface multiplie.
//!
//! **Sa teinte ne s'accorde pas au décor, et c'est délibéré** : les planches
//! d'aujourd'hui sont provisoires, donc un réglage pris sur elles serait à refaire
//! avec l'éclairage. Ce qui tient quel que soit l'habillage est la clarté — un
//! élément clair se lit sur un fond sombre, et le registre sombre du jeu la garantit.

use screengine_play::Output;

use super::{EDGE, PLATE, block, scale};

/// L'épaisseur de la jauge, en pixels à la hauteur d'ouverture.
const THICK: u32 = 7;

/// Sa marge aux bords gauche et bas, en pixels à la hauteur d'ouverture.
const INSET: u32 = 8;

/// En combien de parts de la largeur du tampon la jauge tient.
///
/// **Un quart, et ce qui le décide est le cran** : une morsure retire un dixième de
/// la vie, donc la jauge a besoin d'au moins dix pixels utiles pour que le retrait se
/// voie. À la résolution interne du jeu, le quart en fait cent soixante.
const SHARE: u32 = 4;

/// Le remplissage : ce qui reste de vie.
const FILL: [u8; 4] = [0xD0, 0x3C, 0x30, 0xFF];

// Le liseré se dessine un facteur avant la jauge, donc la marge ne peut pas être
// nulle : sur un entier non signé, elle déborderait par le haut et la jauge
// partirait à l'autre bout de la ligne. Un facteur suffit, la marge étant comptée
// dans la même unité — c'est ce qui rend la garde indépendante de la résolution.
const _: () = assert!(INSET >= 1);

#[cfg(test)]
mod tests;

/// Dessine la jauge en bas à gauche, remplie de cette part.
///
/// La part est attendue de zéro à un, ce que la course garantit : au-delà, le
/// remplissage dépasserait son fond sans que le tampon l'écrête.
///
/// **Un tampon trop petit ne reçoit rien**, et les deux cotes se refusent pour deux
/// raisons distinctes. En hauteur, le coin bas se calcule par soustraction : sur un
/// entier non signé, une hauteur plus faible que la marge ne descend pas sous zéro —
/// elle déborde par le haut, et la panique arriverait loin de sa cause. En largeur,
/// rien ne paniquerait, le tampon sautant ce qui dépasse ; ce qu'on éviterait est
/// pire — un liseré sans son fond, soit une jauge tronquée qui se lit comme une jauge
/// vide.
pub fn draw(output: &mut Output<'_>, share: f32) {
    // Le liseré prend le facteur comme le reste : à un pixel, il disparaîtrait sur une
    // image que l'on agrandit, et c'est lui qui dit où une jauge vide s'arrête.
    let unit = scale(output.height());
    let (thick, inset) = (THICK * unit, INSET * unit);

    let span = output.width() / SHARE;
    if output.width() < inset + span + unit || output.height() < thick + 2 * inset {
        return;
    }

    let left = inset;
    let top = output.height() - inset - thick;

    let edge = 2 * unit;
    block(
        output,
        left - unit,
        top - unit,
        span + edge,
        thick + edge,
        EDGE,
    );
    block(output, left, top, span, thick, PLATE);
    block(output, left, top, (span as f32 * share) as u32, thick, FILL);
}

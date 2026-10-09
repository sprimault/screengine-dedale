// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! La jauge de vie : un fond, un liseré, et un remplissage qui dit ce qui reste.
//!
//! **L'ancrage se prend du tampon à chaque image, les longueurs restent en pixels.**
//! La résolution interne n'est pas celle de la fenêtre et peut changer en cours de
//! partie, donc le coin se recalcule ; mais une épaisseur qui suivrait la hauteur du
//! tampon maigrirait avec la résolution, et c'est l'inverse de ce qu'une interface à
//! basse résolution demande — un trait d'un pixel y disparaît. C'est le même partage
//! que le réticule, et c'est le jour où la résolution deviendrait réglable en jouant
//! que tout le HUD reposera la question.
//!
//! **Sa teinte ne s'accorde pas au décor, et c'est délibéré** : les planches
//! d'aujourd'hui sont provisoires, donc un réglage pris sur elles serait à refaire
//! avec l'éclairage. Ce qui tient quel que soit l'habillage est la clarté — un
//! élément clair se lit sur un fond sombre, et le registre sombre du jeu la garantit.

use screengine_play::Output;

use super::{EDGE, PLATE, block};

/// L'épaisseur de la jauge, en pixels.
const THICK: u32 = 7;

/// Sa marge aux bords gauche et bas, en pixels.
const INSET: u32 = 8;

/// En combien de parts de la largeur du tampon la jauge tient.
///
/// **Un quart, et ce qui le décide est le cran** : une morsure retire un dixième de
/// la vie, donc la jauge a besoin d'au moins dix pixels utiles pour que le retrait se
/// voie. À la résolution interne du jeu, le quart en fait cent soixante.
const SHARE: u32 = 4;

/// Le remplissage : ce qui reste de vie.
const FILL: [u8; 4] = [0xD0, 0x3C, 0x30, 0xFF];

// Le liseré se dessine un pixel avant la jauge, donc la marge ne peut pas être
// nulle : sur un entier non signé, elle déborderait par le haut et la jauge
// partirait à l'autre bout de la ligne.
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
    let span = output.width() / SHARE;
    if output.width() < INSET + span + 1 || output.height() < THICK + 2 * INSET {
        return;
    }

    let left = INSET;
    let top = output.height() - INSET - THICK;

    block(output, left - 1, top - 1, span + 2, THICK + 2, EDGE);
    block(output, left, top, span, THICK, PLATE);
    block(output, left, top, (span as f32 * share) as u32, THICK, FILL);
}

// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Ce qui se dessine en coordonnées d'**écran**, dans le tampon de sortie.
//!
//! **Jamais par une soumission au moteur**, et c'est un invariant du projet : le
//! moteur ne connaît ni interface ni texte, et son tracé de lignes prend des
//! coordonnées de **monde** — il sert les repères d'un éditeur, pas un réticule. Tout
//! ce qui vit ici est écrit après la fin d'image, dans le rappel de sortie.
//!
//! **La vue de dessus n'en fera pas partie**, et c'est délibéré : elle se trace en
//! coordonnées de monde par le moteur, donc elle aura son propre module. Les ranger
//! ensemble ferait oublier la frontière qui les sépare.

pub mod gauge;
pub mod glyph;
pub mod reticle;
pub mod score;

use screengine_play::Output;

use crate::resolution;

#[cfg(test)]
mod tests;

/// Le facteur par lequel les cotes de l'interface suivent la hauteur du tampon.
///
/// **La résolution interne est réglable en cours de partie**, et c'est ce qui rend
/// cette fonction nécessaire : les cotes du HUD ont été réglées à l'œil sur la hauteur
/// d'ouverture, et laissées en pixels, elles maigriraient d'autant que la résolution
/// monte — une jauge de sept pixels se lit trois fois plus fine sur une image trois
/// fois plus haute. Chaque élément tire donc son facteur du tampon qu'il reçoit, comme
/// il en tire déjà son ancrage.
///
/// **Entier, et c'est la planche de glyphes qui l'impose** : un texte se dessine en
/// multipliant ses cases par un entier, donc un facteur fractionnaire n'aurait aucune
/// traduction pour le compteur. Le reste de l'interface suit le même pas pour que
/// l'ensemble reste d'un seul tenant.
///
/// **Arrondi au plus proche plutôt que tronqué** : à la troncature, une image de 540
/// de haut — le cas d'une fenêtre entre deux crans — garderait les cotes de 360 et
/// paraîtrait un tiers trop fine. Le pas reste visible quand le facteur change, et il
/// n'y a pas de remède : c'est le prix d'un entier.
///
/// **La hauteur et non la largeur**, parce que c'est elle qui porte l'aspect : une
/// fenêtre ultra-large ne grandit pas ce qu'on lit, elle en écarte seulement les
/// coins.
pub fn scale(height: u32) -> u32 {
    ((height + resolution::BASE.1 / 2) / resolution::BASE.1).max(1)
}

/// Le fond d'un élément d'interface, sous ce qu'il montre.
///
/// **Une seule teinte pour la jauge et le compteur, et c'est la raison de sa place
/// ici** : les deux reposent sur la même ligne au bas de l'écran, à chaque bout, donc
/// un écart entre leurs fonds se lirait comme un défaut. Deux constantes de même
/// valeur divergent au premier réglage.
pub const PLATE: [u8; 4] = [0x20, 0x1C, 0x1C, 0xFF];

/// Le liseré qui détache un élément d'interface du décor.
///
/// **Sans lui, un fond sombre disparaît sur un mur sombre** — et c'est l'ambiance que
/// ce jeu vise. Ce qu'il tient n'est pas le contraste du contenu mais la frontière de
/// l'élément : on doit voir où une jauge vide s'arrête.
pub const EDGE: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

/// Peint un rectangle plein dans le tampon de sortie.
///
/// **Les pixels hors du tampon sont simplement sautés**, et c'est voulu : un élément
/// d'interface posé près d'un bord se découpe de lui-même, sans que chaque appelant
/// ait à borner ses cotes. Le tampon décide, puisque c'est lui qui connaît sa taille.
pub fn block(output: &mut Output<'_>, x: u32, y: u32, width: u32, height: u32, color: [u8; 4]) {
    for row in 0..height {
        for column in 0..width {
            if let Some(pixel) = output.pixel(x + column, y + row) {
                pixel.copy_from_slice(&color);
            }
        }
    }
}

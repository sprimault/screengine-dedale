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
pub mod reticle;

use screengine_play::Output;

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

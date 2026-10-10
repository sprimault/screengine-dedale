// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le tampon d'une épreuve d'écran : ses cotes, son pas de ligne, et sa relecture.
//!
//! **Son pas dépasse sa largeur, et c'est toute sa raison d'être.** Le moteur ne reçoit
//! jamais la longueur du tampon, seulement son pas : un dessin qui confond les deux se
//! décale d'une ligne à l'autre, et aucune épreuve ne le verrait sur un tampon serré.
//!
//! **Son arithmétique est écrite une seconde fois exprès.** Ce qu'une épreuve d'écran
//! mesure passe par le pixel que le tampon prêté donne ; si elle fabriquait aussi son
//! cas par lui, un adressage faux serait partagé par l'oracle et par le sujet, et
//! l'égalité resterait verte. Les accès d'ici n'empruntent donc rien au tampon prêté.

use screengine_play::{BYTES_PER_PIXEL, Output};

/// L'écart entre la largeur d'une image d'épreuve et le pas de son tampon.
///
/// Sa valeur ne compte pas ; qu'il ne soit pas nul, si.
const MARGIN: u32 = 16;

/// Un tampon d'épreuve, peint puis relu.
pub struct Canvas {
    /// Les octets, marge de pas et ligne de garde comprises.
    pixels: Vec<u8>,
    /// La largeur de l'image, qui borne ce que le tampon prêté accepte.
    width: u32,
    /// Sa hauteur, qui la borne de même.
    height: u32,
}

impl Canvas {
    /// Un tampon noir aux cotes demandées.
    ///
    /// **Une ligne de plus que l'image est allouée**, et c'est elle qui permet
    /// d'éprouver qu'un dessin ne déborde pas : sans elle, un débord sortirait du
    /// vecteur au lieu de se relire.
    pub fn new(width: u32, height: u32) -> Self {
        let stride = width + MARGIN;
        Self {
            pixels: vec![0; (stride * (height + 1)) as usize * BYTES_PER_PIXEL],
            width,
            height,
        }
    }

    /// La largeur de l'image.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Sa hauteur.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Le pas d'une ligne.
    fn stride(&self) -> u32 {
        self.width + MARGIN
    }

    /// Le rang du premier octet d'un pixel, adressé par le pas.
    fn at(&self, x: u32, y: u32) -> usize {
        (y as usize * self.stride() as usize + x as usize) * BYTES_PER_PIXEL
    }

    /// Le tampon prêté à ce qui écrit dedans — ou à ce qui le relit, comme le relevé.
    pub fn output(&mut self) -> Output<'_> {
        let stride = self.stride();
        Output::new(&mut self.pixels, self.width, self.height, stride)
    }

    /// Les octets entiers, marge et ligne de garde comprises.
    ///
    /// Ce qu'ils servent est le tout ou rien : un tampon qui n'a rien reçu, ou qui a
    /// reçu quelque chose.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Les quatre octets d'un pixel.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = self.at(x, y);
        self.pixels[at..at + BYTES_PER_PIXEL]
            .try_into()
            .expect("quatre octets")
    }

    /// Pose un pixel.
    pub fn set(&mut self, x: u32, y: u32, colour: [u8; 4]) {
        let at = self.at(x, y);
        self.pixels[at..at + BYTES_PER_PIXEL].copy_from_slice(&colour);
    }

    /// Peint tout le tampon, marge comprise.
    pub fn fill(&mut self, colour: [u8; 4]) {
        for pixel in self.pixels.chunks_exact_mut(BYTES_PER_PIXEL) {
            pixel.copy_from_slice(&colour);
        }
    }
}

// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du réticule : son centre, son vide, et ce qu'il refuse.
//!
//! **Ce qui s'éprouve est l'arithmétique de l'adressage**, et rien de l'aspect : qu'un
//! réticule soit joli ou lisible se juge à l'écran, mais qu'il tombe au centre du
//! tampon et pas à côté est une fonction d'entrées vers des sorties.
//!
//! **Le symptôme guetté est une cote ancrée sur une constante** : la résolution interne
//! n'est pas celle de la fenêtre et peut changer en cours de jeu, donc le centre se
//! reprend du tampon à chaque image. Deux tampons de tailles différentes sont ce qui
//! l'attrape.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;

/// Le pas de ligne d'un tampon d'épreuve, **en pixels**, et plus large que l'image.
///
/// **Délibérément différent de la largeur, et c'est ce qui donne son mordant à ce
/// fichier** : le moteur ne reçoit jamais la longueur du tampon, seulement son pas, et
/// confondre les deux est le piège qu'il nomme lui-même. Un adressage écrit sur la
/// largeur passerait inaperçu sur un tampon serré et dériverait d'une ligne par ligne
/// sur celui-ci.
fn stride(width: u32) -> u32 {
    width + 16
}

/// Un tampon noir, et le réticule dessiné dedans.
///
/// Rend les octets, que les épreuves relisent par leurs coordonnées.
fn drawn(width: u32, height: u32) -> Vec<u8> {
    let mut buffer = vec![0u8; (stride(width) * height) as usize * 4];
    let mut output = Output::new(&mut buffer, width, height, stride(width));
    draw(&mut output);
    buffer
}

/// Le pixel relu dans un tampon d'épreuve, adressé par le **pas** et non la largeur.
fn pixel(buffer: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * stride(width) + x) * 4) as usize;
    [buffer[at], buffer[at + 1], buffer[at + 2], buffer[at + 3]]
}

/// Le centre reste vide, et les quatre bras l'entourent.
///
/// **Le vide est la propriété qui compte** : ce qu'on vise fait quelques pixels, et un
/// trait qui passe dessus le cache au moment où on le regarde. Les quatre bras sont
/// vérifiés ensemble parce qu'un seul manquant ne se verrait pas en jouant — on
/// croirait à un réticule asymétrique voulu.
#[test]
fn le_centre_reste_vide_entre_quatre_bras() {
    let (width, height) = (64, 48);
    let buffer = drawn(width, height);
    let (cx, cy) = (width / 2, height / 2);

    assert_eq!(
        pixel(&buffer, width, cx, cy),
        [0, 0, 0, 0],
        "le centre du réticule est peint, donc il cache ce qu'on vise"
    );

    // Au premier pixel de chaque bras, le blanc : c'est `GAP` pixels du centre.
    for (x, y, side) in [
        (cx - GAP - 1, cy, "à gauche"),
        (cx + GAP + 1, cy, "à droite"),
        (cx, cy - GAP - 1, "au-dessus"),
        (cx, cy + GAP + 1, "en dessous"),
    ] {
        assert_eq!(
            pixel(&buffer, width, x, y),
            INK,
            "le bras {side} ne commence pas à {GAP} pixels du centre"
        );
    }
}

/// Le centre vient du tampon, et non d'une constante.
///
/// **C'est le symptôme que le projet guette depuis que le HUD est en vue** : une cote
/// d'écran ancrée sur une constante est juste le jour où on l'écrit et fausse au
/// premier changement de résolution, sans que rien ne le dise. Deux tampons de
/// proportions différentes l'attrapent, là où un seul laisserait passer n'importe
/// quelle constante qui tombe juste.
#[test]
fn le_centre_vient_du_tampon() {
    for (width, height) in [(64, 48), (128, 72), (40, 200)] {
        let buffer = drawn(width, height);
        let (cx, cy) = (width / 2, height / 2);

        assert_eq!(
            pixel(&buffer, width, cx + GAP + 1, cy),
            INK,
            "sur un tampon de {width}×{height}, le bras droit n'est pas au centre"
        );
        assert_eq!(
            pixel(&buffer, width, cx, cy + GAP + 1),
            INK,
            "sur un tampon de {width}×{height}, le bras du bas n'est pas au centre"
        );
    }
}

/// Un liseré sombre borde chaque bras, pour qu'il se détache d'un fond clair.
///
/// **Sans lui, le réticule disparaît sur une brique claire**, et c'est exactement le
/// décor de ce jeu. Ce qui se vérifie est qu'il est **sous** le blanc et non à sa
/// place : le pixel juste au-dessus du bras horizontal doit être noir, et le bras
/// lui-même blanc.
#[test]
fn un_lisere_borde_chaque_bras() {
    let (width, height) = (64, 48);
    let buffer = drawn(width, height);
    let (cx, cy) = (width / 2, height / 2);

    let arm = cx + GAP + 1;
    assert_eq!(pixel(&buffer, width, arm, cy), INK);
    assert_eq!(
        pixel(&buffer, width, arm, cy - 1),
        EDGE,
        "le bras droit n'a pas de liseré au-dessus"
    );
    assert_eq!(
        pixel(&buffer, width, arm, cy + 1),
        EDGE,
        "le bras droit n'a pas de liseré en dessous"
    );
}

/// Un tampon trop petit ne reçoit rien, plutôt que de déborder.
///
/// **Les cotes viennent de l'hôte**, et le bras se soustrait du centre : sur un entier
/// non signé, un centre plus petit que le bras ne descend pas sous zéro — il déborde
/// par le haut, et la panique arriverait loin de sa cause. Le refus est donc en tête de
/// la fonction, et il se vérifie sur le dernier côté qui ne passe pas.
#[test]
fn un_tampon_trop_petit_ne_recoit_rien() {
    let side = SPAN - 1;
    let buffer = drawn(side, side);
    assert!(
        buffer.iter().all(|byte| *byte == 0),
        "un tampon de {side}×{side} a reçu des pixels alors qu'il faut {SPAN}"
    );

    // Et le premier qui passe en reçoit : sans ce second cas, un refus de tout
    // passerait.
    let buffer = drawn(SPAN, SPAN);
    assert!(
        buffer.iter().any(|byte| *byte != 0),
        "un tampon de {SPAN}×{SPAN} n'a rien reçu alors qu'il est à la taille"
    );
}

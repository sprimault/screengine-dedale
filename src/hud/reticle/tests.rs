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
//! **Les tampons d'ici sont tous sous la hauteur d'ouverture, donc au facteur un**, et
//! les cotes attendues sont celles des constantes sans multiplication. C'est voulu :
//! l'arithmétique du centre se lit mieux sur le cas neutre, et le facteur a son
//! épreuve à part, qui est la seule à payer un grand tampon.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use crate::test_support::Canvas;

/// Le facteur des tampons de ces épreuves, sous la hauteur d'ouverture.
const UNIT: u32 = 1;

/// Un tampon noir, et le réticule dessiné dedans.
///
/// Rend le tampon, que les épreuves relisent par leurs coordonnées.
fn drawn(width: u32, height: u32) -> Canvas {
    let mut canvas = Canvas::new(width, height);
    draw(&mut canvas.output());
    canvas
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
    let canvas = drawn(width, height);
    let (cx, cy) = (width / 2, height / 2);

    assert_eq!(
        canvas.pixel(cx, cy),
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
            canvas.pixel(x, y),
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
        let canvas = drawn(width, height);
        let (cx, cy) = (width / 2, height / 2);

        assert_eq!(
            canvas.pixel(cx + GAP + 1, cy),
            INK,
            "sur un tampon de {width}×{height}, le bras droit n'est pas au centre"
        );
        assert_eq!(
            canvas.pixel(cx, cy + GAP + 1),
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
    let canvas = drawn(width, height);
    let (cx, cy) = (width / 2, height / 2);

    let arm = cx + GAP + 1;
    assert_eq!(canvas.pixel(arm, cy), INK);
    assert_eq!(
        canvas.pixel(arm, cy - 1),
        EDGE,
        "le bras droit n'a pas de liseré au-dessus"
    );
    assert_eq!(
        canvas.pixel(arm, cy + 1),
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
    let reach = span(UNIT);
    let side = reach - 1;
    let canvas = drawn(side, side);
    assert!(
        canvas.pixels().iter().all(|byte| *byte == 0),
        "un tampon de {side}×{side} a reçu des pixels alors qu'il faut {reach}"
    );

    // Et le premier qui passe en reçoit : sans ce second cas, un refus de tout
    // passerait.
    let canvas = drawn(reach, reach);
    assert!(
        canvas.pixels().iter().any(|byte| *byte != 0),
        "un tampon de {reach}×{reach} n'a rien reçu alors qu'il est à la taille"
    );
}

/// Le réticule grandit avec le tampon, et son vide avec lui.
///
/// **C'est la clause que ce lot ajoute**, et le symptôme qu'elle garde se voit sans se
/// mesurer : un réticule figé en pixels se perd au centre d'une image trois fois plus
/// haute, là où il doit dire où part le rayon.
///
/// **Le vide est mesuré autant que le bras**, et c'est ce qui fait mordre l'épreuve :
/// multiplier la longueur sans le vide rapprocherait les quatre bras du centre, donc
/// recouvrirait exactement ce que le vide existe pour laisser voir.
///
/// La hauteur est celle d'un facteur deux, et le tampon est le plus petit qui la porte
/// — une épreuve de dessin n'a pas à payer une image entière.
#[test]
fn le_reticule_suit_la_hauteur_du_tampon() {
    let height = 2 * crate::resolution::BASE.1;
    let canvas = drawn(span(2), height);
    let unit = crate::hud::scale(height);
    assert_eq!(unit, 2, "la hauteur choisie ne donne pas le facteur deux");

    let (cx, cy) = (span(2) / 2, height / 2);

    // Les trois pixels qui se suivent sur la droite du centre : le vide, le liseré qui
    // précède le bras, puis le bras. Le dernier seul laisserait passer un bras allongé
    // qui aurait mangé son vide, et le liseré est ce qui dit que lui aussi a suivi.
    let gap = GAP * unit;
    assert_eq!(
        canvas.pixel(cx + gap - unit, cy),
        [0, 0, 0, 0],
        "le vide n'a pas suivi le facteur : quelque chose est peint à {} du centre",
        gap - unit
    );
    assert_eq!(
        canvas.pixel(cx + gap, cy),
        EDGE,
        "le liseré du bras droit n'est pas à {gap} pixels du centre"
    );
    assert_eq!(
        canvas.pixel(cx + gap + unit, cy),
        INK,
        "le bras droit ne commence pas à {} pixels du centre",
        gap + unit
    );

    // Et il est épais du facteur : un trait d'un pixel se perdrait sur une image que
    // l'on agrandit.
    assert_eq!(
        canvas.pixel(cx + GAP * unit + unit, cy + unit - 1),
        INK,
        "le bras droit n'est pas épais de {unit} pixels"
    );
}

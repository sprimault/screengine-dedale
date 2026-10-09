// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de la jauge : son ancrage, sa part, son liseré, et ce qu'elle refuse.
//!
//! **Ce qui s'éprouve est l'arithmétique**, et rien de l'aspect : qu'une jauge soit
//! lisible, bien placée ou de la bonne teinte se juge à l'écran, mais qu'une part de
//! moitié remplisse la moitié est une fonction d'entrées vers des sorties.
//!
//! **Le symptôme guetté est une cote ancrée sur une constante** : la résolution
//! interne n'est pas celle de la fenêtre et peut changer en cours de jeu, donc le coin
//! bas se reprend du tampon à chaque image. Deux tampons de tailles différentes sont
//! ce qui l'attrape.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use crate::test_support::Canvas;

/// Un tampon noir, et la jauge dessinée dedans à cette part.
fn drawn(width: u32, height: u32, share: f32) -> Canvas {
    let mut canvas = Canvas::new(width, height);
    draw(&mut canvas.output(), share);
    canvas
}

/// La jauge se pose au coin bas gauche, et son coin vient du tampon.
///
/// **Deux tampons de proportions différentes, et c'est ce qui le fait mordre** : une
/// ordonnée écrite en constante tombe juste le jour où on l'écrit et se décolle du bas
/// au premier changement de résolution, sans que rien ne le dise. Ce qui est vérifié
/// est le pixel le plus bas et le plus à gauche du remplissage, sur une jauge pleine.
#[test]
fn le_coin_de_la_jauge_vient_du_tampon() {
    for (width, height) in [(320, 180), (160, 100), (64, 48)] {
        let canvas = drawn(width, height, 1.0);
        let bottom = height - INSET - 1;

        assert_eq!(
            canvas.pixel(INSET, bottom),
            FILL,
            "sur un tampon de {width}×{height}, la jauge n'occupe pas son coin bas gauche"
        );
        assert_eq!(
            canvas.pixel(INSET, bottom + 1),
            EDGE,
            "sur un tampon de {width}×{height}, la jauge n'est pas bordée en dessous"
        );
    }
}

/// Une part se traduit en largeur de remplissage, et le reste montre le fond.
///
/// **C'est la propriété de la jauge**, et la seule : elle existe pour que ce qui reste
/// de vie se lise d'un coup d'œil. Trois parts plutôt qu'une — le plein et le vide
/// passeraient une largeur qui ignorerait la part, et la moitié seule passerait une
/// part prise à l'envers.
#[test]
fn une_part_se_traduit_en_largeur() {
    let (width, height) = (320, 180);
    let span = width / SHARE;
    let row = height - INSET - THICK;

    for (share, filled) in [(0.0, 0), (0.25, span / 4), (0.5, span / 2), (1.0, span)] {
        let canvas = drawn(width, height, share);

        if filled > 0 {
            assert_eq!(
                canvas.pixel(INSET + filled - 1, row),
                FILL,
                "à la part {share}, le dernier pixel rempli ne l'est pas"
            );
        }
        if filled < span {
            assert_eq!(
                canvas.pixel(INSET + filled, row),
                PLATE,
                "à la part {share}, le remplissage dépasse sa largeur"
            );
        }
    }
}

/// Le remplissage ne dépasse pas son fond, à part pleine.
///
/// **La borne par la droite n'a pas d'autre garde** : le fond et le remplissage
/// partent du même bord, donc une largeur trop grande ne se verrait pas à gauche, et
/// le tampon est trop large pour l'écrêter. C'est le liseré du bord droit qui le dit.
#[test]
fn une_jauge_pleine_s_arrete_a_son_bord() {
    let (width, height) = (320, 180);
    let span = width / SHARE;
    let canvas = drawn(width, height, 1.0);
    let row = height - INSET - THICK;

    assert_eq!(canvas.pixel(INSET + span - 1, row), FILL);
    assert_eq!(
        canvas.pixel(INSET + span, row),
        EDGE,
        "le remplissage a mangé le liseré de droite"
    );
}

/// Un liseré sombre borde la jauge, pour qu'elle se détache d'un fond clair.
///
/// **Sans lui, la jauge disparaît sur une brique claire**, et c'est exactement le
/// décor de ce jeu. Ce qui se vérifie est qu'il est **autour** et non à la place : le
/// pixel juste à gauche du remplissage est sombre, et le remplissage lui-même clair.
#[test]
fn un_lisere_borde_la_jauge() {
    let (width, height) = (320, 180);
    let canvas = drawn(width, height, 1.0);
    let row = height - INSET - THICK;

    assert_eq!(canvas.pixel(INSET, row), FILL);
    assert_eq!(
        canvas.pixel(INSET - 1, row),
        EDGE,
        "la jauge n'a pas de liseré à gauche"
    );
    assert_eq!(
        canvas.pixel(INSET, row - 1),
        EDGE,
        "la jauge n'a pas de liseré au-dessus"
    );
}

/// Un tampon trop petit ne reçoit rien, plutôt que de déborder.
///
/// **Les deux cotes se refusent, et pour deux raisons.** En hauteur, le coin bas se
/// calcule par soustraction : sur un entier non signé, une hauteur plus faible que la
/// marge déborde par le haut au lieu de descendre sous zéro, et la panique arriverait
/// loin de sa cause. En largeur, rien ne paniquerait — le tampon saute ce qui dépasse
/// —, et ce qui est en jeu est un liseré sans son fond, qui se lit comme une jauge
/// vide.
///
/// **Chaque refus se vérifie avec le premier cas qui passe** : sans lui, un refus de
/// tout passerait. Et la largeur minimale ne se recopie pas, elle se cherche : elle
/// dépend de trois constantes dont l'une par une division, donc un nombre écrit ici
/// serait juste aujourd'hui et faux au premier réglage.
#[test]
fn un_tampon_trop_petit_ne_recoit_rien() {
    let short = THICK + 2 * INSET - 1;
    let canvas = drawn(320, short, 1.0);
    assert!(
        canvas.pixels().iter().all(|byte| *byte == 0),
        "un tampon de {short} de haut a reçu des pixels alors qu'il en faut {}",
        THICK + 2 * INSET
    );

    let canvas = drawn(320, short + 1, 1.0);
    assert!(
        canvas.pixels().iter().any(|byte| *byte != 0),
        "un tampon de {} de haut n'a rien reçu alors qu'il est à la taille",
        short + 1
    );

    let narrow = (1..64)
        .find(|width| {
            drawn(*width, 180, 1.0)
                .pixels()
                .iter()
                .any(|byte| *byte != 0)
        })
        .expect("une largeur sous soixante-quatre porte la jauge");
    assert!(
        drawn(narrow - 1, 180, 1.0)
            .pixels()
            .iter()
            .all(|byte| *byte == 0),
        "un tampon de {} de large a reçu une jauge qui n'y tient pas",
        narrow - 1
    );
}

// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du compteur de munitions : son accolement à la jauge, la forme de son
//! carré, la silhouette de sa cartouche, et ce qu'il refuse.
//!
//! **Ce qui s'éprouve est l'arithmétique du cadrage**, et rien de l'aspect : qu'une
//! cartouche se reconnaisse à vingt pixels se juge à l'écran, mais qu'elle coiffe le
//! nombre sans le toucher et que le carré soit carré sont des fonctions d'entrées vers
//! des sorties.
//!
//! **Rien ici ne suppose où l'encre d'un chiffre commence dans sa case** — même clause
//! que les épreuves du score : la planche dessine son espacement, donc la dernière
//! colonne encrée s'arrête avant l'avance. Tout se mesure sur des étendues et des bandes
//! vides, jamais sur une empreinte au pixel près.
//!
//! **Et la cartouche ne se distingue pas du nombre par la teinte**, les deux étant à
//! l'encre : ce qui les sépare dans ces épreuves est la géométrie — la bande vide entre
//! eux, et l'étagement de l'ogive.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use crate::test_support::Canvas;

/// Un tampon noir, et le compteur dessiné dedans à ce compte.
fn drawn(width: u32, height: u32, ammo: u32) -> Canvas {
    let glyphs = Glyphs::new();
    let mut canvas = Canvas::new(width, height);
    draw(&mut canvas.output(), &glyphs, ammo);
    canvas
}

/// Les abscisses extrêmes des pixels de cette teinte, s'il y en a.
fn span(canvas: &Canvas, colour: [u8; 4]) -> Option<(u32, u32)> {
    let mut found: Option<(u32, u32)> = None;
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            if canvas.pixel(x, y) == colour {
                found = Some(match found {
                    None => (x, x),
                    Some((low, high)) => (low.min(x), high.max(x)),
                });
            }
        }
    }
    found
}

/// Les ordonnées extrêmes des pixels de cette teinte, s'il y en a.
fn rows(canvas: &Canvas, colour: [u8; 4]) -> Option<(u32, u32)> {
    let mut found: Option<(u32, u32)> = None;
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            if canvas.pixel(x, y) == colour {
                found = Some(match found {
                    None => (y, y),
                    Some((low, high)) => (low.min(y), high.max(y)),
                });
            }
        }
    }
    found
}

/// L'étendue de l'encre sur cette ligne, en nombre de colonnes.
fn ink_on(canvas: &Canvas, y: u32) -> u32 {
    (0..canvas.width())
        .filter(|x| canvas.pixel(*x, y) == glyph::INK)
        .count() as u32
}

/// Le carré se pose après la jauge, et ne mord pas dessus.
///
/// **C'est la propriété qui justifie [`gauge::right`]** : la jauge occupe le bas gauche
/// jusqu'à son liseré, et un carré qui partirait d'une constante recouvrirait son
/// extrémité dès qu'on règle sa longueur. Ce qui est vérifié est que rien du compteur
/// n'apparaît avant ce bord, liseré compris.
///
/// **Deux facteurs d'interface**, parce que l'accolement est un produit : à facteur deux,
/// une cote oubliée dans le calcul de l'écart ferait chevaucher les deux plaques sans
/// qu'un corpus à facteur un le montre.
#[test]
fn le_carre_se_pose_apres_la_jauge() {
    for (width, height) in [(320, 180), (1280, 720)] {
        let canvas = drawn(width, height, 24);
        let unit = scale(height);
        let edge = span(&canvas, EDGE).expect("le carré n'a pas de liseré");

        assert!(
            edge.0 >= gauge::right(height),
            "sur {width}×{height}, le carré commence en {} alors que la jauge finit en {}",
            edge.0,
            gauge::right(height)
        );
        assert_eq!(
            edge.0,
            gauge::right(height) + APART * unit - unit,
            "sur {width}×{height}, l'écart à la jauge n'est pas celui qu'on a réglé"
        );
    }
}

/// La plaque est aussi haute que large.
///
/// **C'est la forme qui a été demandée, et elle ne va pas de soi** : le contenu est plus
/// large que haut — trois rangs de chiffres contre une cartouche et une ligne —, donc une
/// plaque taillée sur son contenu serait un bandeau. Le côté vient de la largeur, et la
/// hauteur s'y aligne.
#[test]
fn la_plaque_est_carree() {
    for (width, height) in [(320, 180), (1280, 720)] {
        let canvas = drawn(width, height, 24);
        let (low, high) = span(&canvas, PLATE).expect("pas de plaque");
        let (top, bottom) = rows(&canvas, PLATE).expect("pas de plaque");

        assert_eq!(
            high - low,
            bottom - top,
            "sur {width}×{height}, la plaque fait {} de large pour {} de haut",
            high - low + 1,
            bottom - top + 1
        );
    }
}

/// Une bande vide sépare la cartouche du nombre.
///
/// **Sans elle, les deux se touchent et la silhouette devient illisible** : le corps de
/// la cartouche et le haut des chiffres sont à la même encre, donc collés ils forment une
/// seule tache. C'est ce que règle l'écart, et c'est la seule façon de l'éprouver sans
/// distinguer les deux par la teinte — ils n'en ont qu'une.
///
/// **Mesurée entre la première et la dernière ligne encrée**, donc à l'intérieur du
/// contenu : une bande au-dessus ou au-dessous ne dirait rien.
#[test]
fn une_bande_vide_separe_la_cartouche_du_nombre() {
    let canvas = drawn(320, 180, 24);
    let (first, last) = rows(&canvas, glyph::INK).expect("pas d'encre");

    let empty = (first..=last).filter(|y| ink_on(&canvas, *y) == 0).count();
    assert!(
        empty > 0,
        "la cartouche et le nombre se touchent, sur {} lignes d'encre continue",
        last - first + 1
    );
}

/// L'ogive de la cartouche est plus étroite que son corps.
///
/// **C'est ce qui la fait lire comme une cartouche** : un rectangle plein serait un
/// jeton, et c'est le seul détail de silhouette qu'une taille de vingt pixels autorise.
/// Les deux paliers se mesurent sur la première ligne encrée et sur le corps.
#[test]
fn l_ogive_est_plus_etroite_que_le_corps() {
    let canvas = drawn(320, 180, 24);
    let (first, _) = rows(&canvas, glyph::INK).expect("pas d'encre");

    let tip = ink_on(&canvas, first);
    let body = ink_on(&canvas, first + TALL - 1);
    assert!(
        tip < body,
        "la pointe fait {tip} colonnes et le corps {body} : l'ogive ne s'étage pas"
    );
}

/// Le carré garde son côté sous trois rangs, et s'étend au-delà.
///
/// **Les deux moitiés de la même clause** : un côté qui ne bouge pas est ce qui tient le
/// carré en place quand le compte descend, et l'extension est le bon échec quand un
/// compte dépasse ce qu'on avait prévu — mieux qu'un chiffre tronqué, qui se lirait comme
/// un autre nombre.
#[test]
fn le_carre_tient_trois_rangs_puis_s_etend() {
    let side = |ammo| {
        let (low, high) = span(&drawn(320, 180, ammo), PLATE).expect("pas de plaque");
        high - low
    };

    assert_eq!(side(0), side(999), "le carré bouge sous trois rangs");
    assert!(
        side(1000) > side(999),
        "un compte à quatre rangs tronque au lieu d'étendre le carré"
    );
}

/// Un tampon trop petit ne reçoit rien, plutôt que de déborder.
///
/// **Les deux cotes se refusent, et pour deux raisons distinctes.** En hauteur, le coin
/// bas se calcule par soustraction, donc sur un entier non signé une hauteur plus faible
/// que la marge déborde par le haut au lieu de descendre sous zéro. En largeur, ce qu'on
/// évite est une plaque amputée qui se lirait comme un compteur vide.
///
/// **La cote minimale ne se recopie pas, elle se cherche** : elle dépend de la jauge, de
/// l'écart et de la planche de glyphes, donc un nombre écrit ici serait juste aujourd'hui
/// et faux au premier réglage.
#[test]
fn un_tampon_trop_petit_ne_recoit_rien() {
    let painted = |width, height| {
        drawn(width, height, 24)
            .pixels()
            .iter()
            .any(|byte| *byte != 0)
    };

    let narrow = (1..640)
        .find(|width| painted(*width, 180))
        .expect("une largeur sous 640 porte le compteur");
    assert!(
        !painted(narrow - 1, 180),
        "un tampon de {} de large a reçu un compteur qui n'y tient pas",
        narrow - 1
    );

    let short = (1..180)
        .find(|height| painted(640, *height))
        .expect("une hauteur sous 180 porte le compteur");
    assert!(
        !painted(640, short - 1),
        "un tampon de {} de haut a reçu un compteur qui n'y tient pas",
        short - 1
    );
}

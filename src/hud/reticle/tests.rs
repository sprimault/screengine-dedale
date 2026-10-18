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
//! **Les cotes attendues sont celles des constantes, sans multiplication**, et c'est
//! ce que le module garantit : le réticule garde sa taille en pixels quelle que soit la
//! résolution, pour garder sa taille à l'écran quand la fenêtre grandit. Une épreuve
//! qui dériverait ses attentes de la hauteur du tampon laisserait passer exactement ce
//! qu'on refuse.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use crate::test_support::Canvas;

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
    let side = SPAN - 1;
    let canvas = drawn(side, side);
    assert!(
        canvas.pixels().iter().all(|byte| *byte == 0),
        "un tampon de {side}×{side} a reçu des pixels alors qu'il faut {SPAN}"
    );

    // Et le premier qui passe en reçoit : sans ce second cas, un refus de tout
    // passerait.
    let canvas = drawn(SPAN, SPAN);
    assert!(
        canvas.pixels().iter().any(|byte| *byte != 0),
        "un tampon de {SPAN}×{SPAN} n'a rien reçu alors qu'il est à la taille"
    );
}

/// Le réticule garde ses cotes quand le tampon grandit, et c'est ce qui lui donne une
/// taille constante à l'écran.
///
/// **La clause se lit à l'envers de l'intuition** : la résolution interne suit la
/// fenêtre à facteur d'agrandissement constant, donc un réticule qui grandirait avec le
/// tampon grandirait deux fois à l'écran — une loupe. Une cote en pixels donne
/// gratuitement l'inverse, et c'est ce qu'on veut d'un repère de visée.
///
/// **Trois hauteurs très écartées**, dont deux bien au-delà de l'ouverture : c'est ce
/// qui attrape une cote dérivée du tampon, qui passerait inaperçue sur deux tampons
/// voisins.
#[test]
fn le_reticule_garde_ses_cotes_sur_un_grand_tampon() {
    for (width, height) in [(64, 48), (1280, 720), (1920, 1080)] {
        let canvas = drawn(width, height);
        let (cx, cy) = (width / 2, height / 2);

        // Le dernier pixel du vide, le liseré, puis le bras : chacun à sa cote de
        // constante, quelle que soit la taille du tampon.
        assert_eq!(
            canvas.pixel(cx + GAP - 1, cy),
            [0, 0, 0, 0],
            "sur {width}×{height}, quelque chose est peint à {} du centre",
            GAP - 1
        );
        assert_eq!(
            canvas.pixel(cx + GAP, cy),
            EDGE,
            "sur {width}×{height}, le liseré n'est pas à {GAP} pixels du centre"
        );
        assert_eq!(
            canvas.pixel(cx + GAP + 1, cy),
            INK,
            "sur {width}×{height}, le bras droit ne commence pas à {} du centre",
            GAP + 1
        );

        // Et il est épais d'un seul pixel : le pixel d'en dessous est son liseré, non
        // son encre. Deux diraient qu'une cote a suivi la hauteur.
        assert_eq!(
            canvas.pixel(cx + GAP + 1, cy + 1),
            EDGE,
            "sur {width}×{height}, le bras droit est plus épais qu'un pixel"
        );
    }
}

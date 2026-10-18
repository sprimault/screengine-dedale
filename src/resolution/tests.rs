// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du choix de résolution interne.
//!
//! **Ce qui se teste ici est une fonction d'entrées vers des sorties**, et rien de
//! plus : la finesse obtenue, la lisibilité du grain et la cadence se regardent, et
//! aucune assertion ne dira qu'une image est agréable. Ce qui est éprouvé est la
//! borne — jamais au-dessus du plafond, jamais sous le plancher — et le facteur, dont
//! une erreur se voit comme des bandes noires plutôt que comme une panique.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;

/// Les fenêtres éprouvées : les cotes courantes d'un bureau, et des formes qui ne
/// tombent pas rond.
///
/// **Les deux dernières sont le corpus qui fait mordre la borne** : une fenêtre très
/// étirée est ce qui sort des cotes du plafond quand le facteur n'est pris que sur un
/// axe, et c'est l'erreur qu'un `max` oublié produit.
const WINDOWS: [(u32, u32); 11] = [
    (0, 0),
    (1, 1),
    (320, 180),
    (1280, 720),
    (1600, 900),
    (1920, 1080),
    (2560, 1600),
    (3840, 2160),
    (6000, 200),
    (300, 5000),
    (9000, 9000),
];

/// La fenêtre d'ouverture rend exactement l'ouverture.
///
/// **C'est ce qui rend le changement invisible au lancement**, et la cote n'est pas
/// choisie : l'étage d'accueil ouvre sa fenêtre à la résolution interne multipliée par
/// deux, et le grain vaut deux, donc la première image est celle d'avant ce lot. Un
/// grain qui ne s'accorderait plus au facteur d'ouverture se verrait ici.
#[test]
fn la_fenetre_d_ouverture_rend_l_ouverture() {
    assert_eq!(fit((2 * BASE.0, 2 * BASE.1)), BASE);
}

/// Le grain divise toute fenêtre, même largement sous le plafond.
///
/// **C'est le parti pris du jeu, et c'est ce que le plafond ne porte pas** : employé
/// comme budget, il laisserait ces trois fenêtres se rendre au pixel près et le grain
/// disparaîtrait de tous les écrans courants. Les cotes attendues sont calculées à la
/// main, pour ne pas confronter la formule à elle-même.
#[test]
fn le_grain_divise_toute_fenetre() {
    assert_eq!(fit((1600, 900)), (800, 450));
    assert_eq!(fit((1920, 1080)), (960, 540));
    assert_eq!(fit((2560, 1440)), (1280, 720));
}

/// Au-delà de ce que le moteur accepte, c'est le plafond qui fait monter le facteur.
///
/// **Le grain couvre tout jusqu'à deux fois la borne**, soit 4096 pixels de côté, donc
/// il faut sortir de là pour éprouver la seconde moitié du `max` — et c'est elle qui
/// rend un refus du moteur impossible. 8192 de large demande le facteur quatre, que le
/// grain seul ne donnerait pas.
#[test]
fn le_plafond_prend_le_relais_du_grain() {
    assert_eq!(fit((8192, 2160)), (2048, 540));
    assert_eq!(fit((2160, 8192)), (540, 2048));
}

/// La résolution ne dépasse jamais le plafond, quelle que soit la forme de la fenêtre.
///
/// **C'est la clause que le moteur refuse**, et elle est la raison de cette fonction :
/// au-delà du maximum réglé à la création, `set_resolution` rend `InvalidArgument` et
/// la résolution ne bouge pas — donc l'image resterait figée à la cote précédente sans
/// que rien ne le dise.
#[test]
fn la_resolution_ne_depasse_jamais_le_plafond() {
    for window in WINDOWS {
        let (width, height) = fit(window);
        assert!(
            width <= CEILING.0 && height <= CEILING.1,
            "fenêtre {window:?} : {width}×{height} passe le plafond {CEILING:?}"
        );
    }
}

/// La résolution tient dans la fenêtre, ou dans le plancher qui l'assume.
///
/// **C'est elle qui rend sûr un plafond venu des écrans**, et c'est la raison de son
/// existence : l'étage d'accueil règle désormais le plafond du contexte sur l'enveloppe
/// des écrans, qui peut être bien plus basse que la borne du moteur. Rien ici ne la
/// connaît — mais une fenêtre ne dépasse pas l'écran qui la porte, donc une résolution
/// bornée par sa fenêtre est bornée par cette enveloppe, et `set_resolution` ne peut
/// pas être refusé.
///
/// **Le plancher est la seule sortie, et elle est couverte** : sous lui, l'image est
/// plus grande que la fenêtre par choix, et le plafond que l'étage d'accueil règle ne
/// descend jamais sous la résolution d'ouverture, elle-même au-dessus du plancher.
#[test]
fn la_resolution_tient_dans_la_fenetre_ou_dans_le_plancher() {
    for window in WINDOWS {
        let (width, height) = fit(window);
        assert!(
            width <= window.0.max(FLOOR.0) && height <= window.1.max(FLOOR.1),
            "fenêtre {window:?} : {width}×{height} déborde de la fenêtre sans être au \
             plancher {FLOOR:?}"
        );
    }
}

/// Et jamais sous le plancher, ni à zéro.
///
/// **Zéro est le cas qui paniquerait**, et il arrive : une fenêtre que le système n'a
/// pas encore dimensionnée rend deux zéros, et le moteur refuse de son côté toute
/// dimension nulle.
#[test]
fn la_resolution_ne_descend_jamais_sous_le_plancher() {
    for window in WINDOWS {
        let (width, height) = fit(window);
        assert!(
            width >= FLOOR.0 && height >= FLOOR.1,
            "fenêtre {window:?} : {width}×{height} passe sous le plancher {FLOOR:?}"
        );
    }
}

/// Le facteur est commun aux deux axes, donc l'aspect de la fenêtre est gardé.
///
/// **C'est ce qui empêche l'image de s'étirer**, et un facteur pris par axe est
/// l'erreur que cela attrape : elle ne paniquerait pas et ne sortirait pas du plafond,
/// mais déformerait tout ce qui est rond. La tolérance est celle de la division
/// entière, un pixel sur chaque axe.
///
/// **Les cas où le plancher mord sont écartés, et il mord plus tôt qu'il n'y paraît** :
/// une fenêtre de 6000 par 200 passe la hauteur du plancher, mais pas après sa division
/// par trois — l'image y est alors plus grande que la fenêtre, et c'est la mise à
/// l'échelle qui garde les proportions. Une cote rendue exactement au plancher est donc
/// le signe à reconnaître, plutôt que de refaire ici le calcul qu'on éprouve.
#[test]
fn l_aspect_de_la_fenetre_est_garde() {
    for window in WINDOWS {
        let (width, height) = fit(window);
        if width == FLOOR.0 || height == FLOOR.1 {
            continue;
        }
        let wanted = window.0 as f32 / window.1 as f32;
        let got = width as f32 / height as f32;
        let slack = (width + 1) as f32 / height as f32 - got;

        assert!(
            (got - wanted).abs() <= slack,
            "fenêtre {window:?} : {width}×{height} rend l'aspect {got} et non {wanted}"
        );
    }
}

/// L'ouverture tient sous le plafond, et le plancher sous l'ouverture.
///
/// **Trois constantes qui se contredisent passeraient la compilation**, et le défaut
/// n'apparaîtrait qu'à l'écran : une ouverture au-dessus du plafond ferait refuser la
/// première hausse, un plancher au-dessus du plafond sortirait du maximum par le
/// chemin même qui sert à ne pas en sortir.
#[test]
fn les_trois_cotes_sont_ordonnees() {
    assert!(FLOOR.0 <= BASE.0 && FLOOR.1 <= BASE.1);
    assert!(BASE.0 <= CEILING.0 && BASE.1 <= CEILING.1);
}

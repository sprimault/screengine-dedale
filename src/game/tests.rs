// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves des commandes de marche.
//!
//! **Tout le reste de ce module se regarde.** La vitesse, le confort dans un coude,
//! la sensibilité de la souris : rien de cela ne se tranche en raisonnant, et aucune
//! épreuve ne dira qu'un couloir est agréable à parcourir. Ce qui est éprouvé ici est
//! la seule part qui soit une fonction d'entrées vers des sorties — la direction d'un
//! pas.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use core::f32;

use super::*;

/// Les lacets éprouvés : les quatre axes, et des valeurs qui ne tombent pas rond.
const YAWS: [f32; 8] = [
    0.0,
    f32::consts::FRAC_PI_2,
    f32::consts::PI,
    -f32::consts::FRAC_PI_2,
    0.3,
    1.7,
    -2.9,
    5.4,
];

/// Un quart de tour de lacet tourne le pas avant d'un quart de tour, et il reste
/// horizontal et unitaire.
///
/// **Le prédicat est géométrique, et c'est ce qui le fait mordre** : comparer le pas
/// au sinus et au cosinus du lacet serait le confronter à l'expression même qu'on
/// éprouve, et ne garderait que le fait que la fonction n'oublie pas de tourner. Un
/// quart de tour, lui, s'applique sans trigonométrie — autour du `+Z`, il envoie
/// `(x, y)` sur `(−y, x)` —, donc il fixe à la fois l'amplitude de la rotation et
/// **son sens**, qui est ce qu'un signe inversé casse.
///
/// Que le tangage ne l'atteigne pas n'a pas d'épreuve et n'en demande pas : la
/// signature ne reçoit que le lacet.
#[test]
fn un_quart_de_tour_de_lacet_tourne_le_pas() {
    for yaw in YAWS {
        let step = walk(yaw, 1.0, 0.0);
        let turned = walk(yaw + std::f32::consts::FRAC_PI_2, 1.0, 0.0);

        let length = step.dot(step).sqrt();
        assert!(
            (length - 1.0).abs() < 1e-6,
            "lacet {yaw} : le pas avant {step:?} mesure {length}"
        );
        assert_eq!(step.z, 0.0, "lacet {yaw} : le pas avant monte");
        assert!(
            (turned.x + step.y).abs() < 1e-6 && (turned.y - step.x).abs() < 1e-6,
            "lacet {yaw} : un quart de tour mène à {turned:?} et non à \
             ({}, {}, 0)",
            -step.y,
            step.x
        );
    }
}

/// Un pas de côté est perpendiculaire au regard, et à droite quand on le demande.
///
/// **Deux exigences, et la seconde est le sens** : perpendiculaire, un pas peut
/// partir des deux côtés, et c'est l'erreur qu'un signe inversé produit — elle se
/// voit à l'écran mais passerait un test de seule orthogonalité. Le produit vectoriel
/// vertical la tranche : il est négatif quand le pas part à droite du regard, dans un
/// repère dont l'axe droit est le `−Y`.
#[test]
fn un_pas_de_cote_est_perpendiculaire() {
    for yaw in YAWS {
        let ahead = walk(yaw, 1.0, 0.0);
        let right = walk(yaw, 0.0, 1.0);

        assert!(
            ahead.dot(right).abs() < 1e-6,
            "lacet {yaw} : le pas de côté {right:?} n'est pas perpendiculaire à {ahead:?}"
        );
        assert!(
            ahead.x * right.y - ahead.y * right.x < 0.0,
            "lacet {yaw} : le pas de côté {right:?} part à gauche de {ahead:?}"
        );
    }
}

/// La diagonale ne va pas plus vite qu'un pas droit.
///
/// **C'est la seule clause de la fonction qui ne soit pas de la trigonométrie**, et
/// elle se mesure : sans elle, marcher de biais avance d'un facteur `√2`, ce qui se
/// joue en permanence et déforme tous les réglages de vitesse.
///
/// **Sans tolérance, et c'est le plafond du noyau qui l'autorise** : la table de
/// racine inverse du moteur est une approximation, mais elle n'arrondit jamais au
/// delà de un sur les quatre diagonales et les lacets éprouvés. Une tolérance
/// laisserait passer exactement ce que l'épreuve refuse — un pas de biais plus
/// rapide qu'un pas droit —, et un dépassement d'un seul ulp serait un vrai
/// changement de vitesse.
#[test]
fn la_diagonale_ne_va_pas_plus_vite() {
    for yaw in YAWS {
        for (ahead, side) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            let step = walk(yaw, ahead, side);
            let length = step.dot(step).sqrt();

            assert!(
                length <= 1.0,
                "lacet {yaw}, pas ({ahead}, {side}) : la diagonale {step:?} \
                 vaut {length}"
            );
        }
    }
}

/// Deux touches opposées tenues ensemble ne demandent rien.
#[test]
fn deux_touches_opposees_s_annulent() {
    assert_eq!(axis(true, true), 0.0);
    assert_eq!(axis(false, false), 0.0);
    assert_eq!(axis(true, false), 1.0);
    assert_eq!(axis(false, true), -1.0);
}

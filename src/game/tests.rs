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

/// Avancer suit le lacet, et regarder ailleurs n'y change rien.
///
/// **Ce n'est pas une paraphrase du calcul** : elle fige la clause qui décide, celle
/// que le tangage ne doit pas atteindre. Un pas construit sur le regard complet
/// passerait cette épreuve à plat et échouerait dès qu'on lève les yeux — d'où le
/// lacet seul en entrée, qui est tout ce que la fonction reçoit.
#[test]
fn avancer_suit_le_lacet() {
    for yaw in YAWS {
        let step = walk(yaw, 1.0, 0.0);
        let (sin, cos) = yaw.sin_cos();

        assert!(
            (step.x - cos).abs() < 1e-6 && (step.y - sin).abs() < 1e-6,
            "lacet {yaw} : le pas avant vaut {step:?} au lieu de ({cos}, {sin}, 0)"
        );
        assert_eq!(
            step.z, 0.0,
            "lacet {yaw} : le pas avant monte de {}",
            step.z
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
#[test]
fn la_diagonale_ne_va_pas_plus_vite() {
    for yaw in YAWS {
        for (ahead, side) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            let step = walk(yaw, ahead, side);
            let length = step.dot(step).sqrt();

            assert!(
                length <= 1.0 + 1e-6,
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

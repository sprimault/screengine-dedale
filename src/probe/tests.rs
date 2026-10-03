// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du relevé.

use super::*;

/// Une caméra libre posée et orientée de travers, comme une marche la laisse.
///
/// Des valeurs qui ne sont ni nulles ni remarquables : un lacet de quart de tour
/// ou un tangage nul passeraient une composition fausse sans rien dire.
fn camera() -> FreeCamera {
    FreeCamera {
        position: Vec3::new(55.56, 13.83, 5.00),
        yaw: 2.317,
        pitch: -0.184,
        ..FreeCamera::new(Vec3::new(55.56, 13.83, 5.00))
    }
}

/// Une pose relevée se rejoue à l'identique.
///
/// **C'est la propriété pour laquelle le relevé note l'orientation**, et elle ne
/// vaut que si rien ne se perd en route : une épreuve qui repose le lacet et le
/// tangage dans une `FreeCamera` doit obtenir l'orientation que le jeu avait, au
/// bit près. Approchée, elle ne reproduirait pas un défaut qui tient à des
/// centièmes d'unité.
#[test]
fn une_pose_relevee_se_rejoue_a_l_identique() {
    let marche = camera();
    let aim = Aim::new(&marche, 1147);

    let rejouee = FreeCamera {
        position: aim.eye,
        yaw: aim.yaw,
        pitch: aim.pitch,
        ..FreeCamera::new(aim.eye)
    };

    assert_eq!(rejouee.camera().position, marche.camera().position);
    assert_eq!(rejouee.camera().orientation, marche.camera().orientation);
}

/// La ligne du relevé donne les angles en degrés.
///
/// **Ils se relisent à l'œil pour se situer dans un couloir**, et c'est la seule
/// raison de les convertir : un lacet de 2,317 radians ne se reconnaît pas, 132,75°
/// se place. L'épreuve tient la conversion autant que le gabarit, parce qu'une
/// ligne illisible ne se remarque pas quand on en relit quatre-vingts.
#[test]
fn la_ligne_donne_les_angles_en_degres() {
    let ligne = Aim::new(&camera(), 1147).to_string();
    assert_eq!(
        ligne,
        "œil (55.56 13.83 5.00), lacet 132.75°, tangage -10.54°, cellule 1147"
    );
}

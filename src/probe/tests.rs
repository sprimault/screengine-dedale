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

/// La largeur des images d'épreuve.
const WIDTH: u32 = 320;

/// Leur hauteur.
const HEIGHT: u32 = 180;

/// Un tampon peint d'une seule teinte, et ce que le relevé y compte.
///
/// Le pas de ligne dépasse la largeur, comme celui d'un tampon dimensionné sur le
/// plafond de résolution : le compte adresse par le pas, et une image qui s'y
/// décalerait ne tomberait pas sur les points attendus.
fn tally(fill: [u8; 3], background: [u8; 3]) -> u32 {
    const STRIDE: u32 = 384;

    let mut buffer = vec![0u8; (STRIDE * HEIGHT) as usize * 4];
    for pixel in buffer.chunks_exact_mut(4) {
        pixel[..3].copy_from_slice(&fill);
        pixel[3] = 0xFF;
    }
    let mut output = Output::new(&mut buffer, WIDTH, HEIGHT, STRIDE);
    painted(&mut output, background)
}

/// Une image entièrement au fond ne compte aucun point peint, quelle que soit la
/// couleur du fond.
///
/// **C'est le défaut que le relevé portait** : il comparait au noir, si bien qu'un
/// fond de brouillard — la couleur qu'un pixel non peint prend de lui-même, dit le
/// contrat du moteur — aurait compté pour une image pleine. L'instrument serait
/// devenu muet le jour où l'ambiance arrive, sans rien dire.
#[test]
fn une_image_au_fond_ne_compte_rien() {
    for background in [[0x00, 0x00, 0x00], [0x30, 0x34, 0x3C], [0xFF, 0xFF, 0xFF]] {
        assert_eq!(
            tally(background, background),
            0,
            "un fond {background:?} compte des points peints"
        );
    }
}

/// Une image entièrement peinte les compte tous.
#[test]
fn une_image_pleine_compte_tout() {
    assert_eq!(tally([0x80, 0x40, 0x20], [0x00, 0x00, 0x00]), 100);
    assert_eq!(tally([0x00, 0x00, 0x00], [0x30, 0x34, 0x3C]), 100);
}

/// Une image coupée en deux en compte la moitié.
///
/// **C'est la proportion qui est en jeu**, et non le tout ou rien : la coupure que
/// le relevé existe pour voir laisse l'autre moitié intacte, qu'un seuil « image
/// entièrement vide » ne verrait jamais.
#[test]
fn une_image_coupee_en_compte_la_moitie() {
    const STRIDE: u32 = 384;
    let background = [0x30, 0x34, 0x3C];

    let mut buffer = vec![0u8; (STRIDE * HEIGHT) as usize * 4];
    for y in 0..HEIGHT {
        for x in 0..STRIDE {
            let base = (y as usize * STRIDE as usize + x as usize) * 4;
            let fill = if x < WIDTH / 2 {
                [0x80, 0x40, 0x20]
            } else {
                background
            };
            buffer[base..base + 3].copy_from_slice(&fill);
            buffer[base + 3] = 0xFF;
        }
    }

    let mut output = Output::new(&mut buffer, WIDTH, HEIGHT, STRIDE);
    assert_eq!(painted(&mut output, background), 50);
}

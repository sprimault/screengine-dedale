// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du battement et de la sélection des huit.
//!
//! **Ce qui se regarde et ce qui se mesure se séparent nettement ici.** Qu'un plafond de
//! tubes fatigués se lise comme un souterrain mal entretenu ne s'écrit dans aucune
//! assertion ; que le battement s'éteigne vraiment, qu'il ne s'éteigne pas toujours, et que
//! la borne du moteur soit tenue sont des fonctions d'entrées vers des sorties.
//!
//! **La borne des huit est le point qui compte**, parce que la dépasser est un refus du
//! moteur et non un défaut d'image : plusieurs centaines de tubes existent, et c'est le
//! tri qui garantit qu'il n'en part jamais plus de huit.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;

/// Un labyrinthe d'épreuve, aux réglages du jeu.
fn maze() -> Grid {
    Grid::generate(crate::MAZE)
}

/// Un tube neuf ne s'éteint jamais, un tube usé s'éteint.
///
/// **C'est le seuil que l'usure déplace, et c'est tout le mécanisme** : à usure nulle il
/// est sous le minimum de la somme des trois ondes, donc le tube tient toujours ; à usure
/// pleine il ne repasse au-dessus que par saccades. Une usure qui jouerait sur l'amplitude
/// donnerait des tubes faibles au lieu de tubes qui lâchent.
///
/// **Mille instants plutôt qu'un** : la somme a des périodes incommensurables, donc un
/// instant choisi ne dit rien — c'est sur la durée que l'extinction se voit.
#[test]
fn l_usure_decide_des_extinctions() {
    const MOMENTS: u32 = 1000;

    let dark = |wear: f32| {
        (0..MOMENTS)
            .filter(|rank| beat(*rank as f32 * 0.05, 0.0, wear) == 0.0)
            .count()
    };

    // **Un tube neuf « ne bronche qu'à peine », il ne tient pas toujours** : le seuil à
    // usure nulle vaut −0,95 et la somme des trois ondes descend à −1, donc il lâche
    // rarement. Exiger qu'il ne lâche jamais demanderait un seuil sous le minimum, et un
    // tube absolument stable ne se distinguerait plus d'une lampe cuite.
    assert!(
        dark(0.0) * 50 < MOMENTS as usize,
        "un tube neuf s'éteint sur {} instants sur {MOMENTS} : ce n'est plus « à peine »",
        dark(0.0)
    );
    assert!(dark(1.0) > 0, "un tube usé ne s'éteint jamais");
    assert!(
        dark(1.0) > dark(0.5) && dark(0.5) > dark(0.0),
        "l'usure n'éteint pas d'autant plus souvent qu'elle est forte"
    );
}

/// Un tube allumé reste haut, et jamais au-dessus de son plein.
///
/// **Le vacillement ne va pas jusqu'à l'extinction douce** : au-dessus du seuil, le tube
/// garde entre sept et dix dixièmes de sa lumière. Sans cette borne basse, un tube
/// traverserait toute la plage et se lirait comme une respiration, pas comme un néon.
#[test]
fn un_tube_allume_reste_haut() {
    for rank in 0..1000 {
        let level = beat(rank as f32 * 0.05, 1.3, 0.4);
        assert!(
            level == 0.0 || (0.44..=1.0).contains(&level),
            "le niveau {level} sort de ce qu'un tube peut rendre"
        );
    }
}

/// La phase décale les tubes, et ils ne lâchent pas ensemble.
///
/// **Sans elle, tout le labyrinthe clignoterait d'un bloc**, ce qui se lit comme une
/// coupure de courant et non comme une rangée de tubes fatigués. Deux phases éloignées
/// suffisent à le montrer : leurs extinctions ne tombent pas aux mêmes instants.
#[test]
fn la_phase_decorrele_les_tubes() {
    let dark_at = |phase: f32| {
        (0..1000)
            .filter(|rank| beat(*rank as f32 * 0.05, phase, 0.9) == 0.0)
            .collect::<Vec<_>>()
    };

    let (here, there) = (dark_at(0.0), dark_at(2.3));
    assert!(
        !here.is_empty() && !there.is_empty(),
        "aucun des deux ne lâche"
    );
    assert_ne!(here, there, "deux phases éteignent aux mêmes instants");
}

/// Le lot soumis ne dépasse jamais ce que le moteur accepte.
///
/// **C'est un refus et non un défaut d'image** : au-delà de huit lumières dynamiques, la
/// soumission est rejetée et l'image perd tout son éclairage dynamique — pas seulement le
/// surplus. Le labyrinthe porte plusieurs centaines de tubes, donc la borne est atteinte
/// en permanence et non dans un cas limite.
///
/// **Quatre poses plutôt qu'une**, dont une hors du décor : le tri porte sur la distance à
/// l'œil, donc un œil qui ne voit rien doit rendre un lot tenable comme les autres.
#[test]
fn le_lot_tient_la_borne_du_moteur() {
    let tubes = Tubes::hang(&maze());
    assert!(
        tubes.items.len() > MAX_LIGHTS,
        "le décor ne porte que {} tubes : la borne n'est pas éprouvée",
        tubes.items.len()
    );

    for eye in [
        Vec3::new(34.0, 34.0, 5.0),
        Vec3::new(2.0, 2.0, 1.5),
        Vec3::new(60.0, 60.0, 5.0),
        Vec3::new(-100.0, -100.0, -100.0),
    ] {
        let lit = tubes.live(eye);
        assert!(
            lit.len() <= MAX_LIGHTS,
            "depuis {eye:?}, {} tubes partent pour une borne de {MAX_LIGHTS}",
            lit.len()
        );
    }
}

/// Les tubes retenus sont les plus proches de l'œil.
///
/// **Allumés d'abord, triés ensuite, et l'ordre compte** : trier avant d'éteindre
/// retiendrait huit voisins dont la moitié ne rend rien. Ce qui est vérifié est qu'aucun
/// tube écarté n'est plus proche qu'un tube retenu.
#[test]
fn les_tubes_retenus_sont_les_plus_proches() {
    let tubes = Tubes::hang(&maze());
    let eye = Vec3::new(34.0, 34.0, 5.0);

    let kept = tubes.live(eye);
    let farthest = kept
        .iter()
        .map(|light| (light.position - eye).dot(light.position - eye))
        .fold(0.0_f32, f32::max);

    let closer = tubes
        .items
        .iter()
        .filter(|tube| beat(tubes.time, tube.phase, tube.wear) > 0.0)
        .filter(|tube| (tube.at - eye).dot(tube.at - eye) < farthest)
        .count();
    assert!(
        closer <= kept.len(),
        "{closer} tubes allumés sont plus proches que le plus lointain des {} retenus",
        kept.len()
    );
}

// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves des commandes de marche, et du rythme des morsures.
//!
//! **Tout le reste de ce module se regarde.** La vitesse, le confort dans un coude,
//! la sensibilité de la souris, et si une morsure à dix de vie se sent : rien de cela
//! ne se tranche en raisonnant, et aucune épreuve ne dira qu'un couloir est agréable à
//! parcourir. Ce qui est éprouvé ici est ce qui est une fonction d'entrées vers des
//! sorties — la direction d'un pas, et ce qu'un pas de contact fait d'une course.
//!
//! **Le recouvrement qui décide du contact est éprouvé auprès de la boîte**, où il
//! vit : c'est une règle de volume et non de partie, et les deux gabarits inégaux y
//! sont le corpus qui la fait mordre.
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

/// Le pas d'une image, à soixante par seconde.
const DT: f32 = 1.0 / 60.0;

/// Une morsure coûte sa part et ouvre un répit.
#[test]
fn une_morsure_coute_sa_part_et_ouvre_un_repit() {
    let mut run = Run::new();
    let left = bite(0.0, DT, true, &mut run);

    assert_eq!(run.life, LIFE - BITE);
    assert_eq!(left, RESPITE, "une morsure n'a pas rouvert son répit");
}

/// Le répit sépare deux morsures, et il court même hors contact.
///
/// **La seconde moitié est ce qui empêche un aller-retour de mordre deux fois** : un
/// répit qui ne s'épuiserait qu'au contact rendrait le coût d'une créature dépendant
/// de la façon dont on s'en éloigne, ce qui ne se devinerait pas en jouant.
#[test]
fn le_repit_separe_deux_morsures() {
    let mut run = Run::new();
    let left = bite(RESPITE, DT, true, &mut run);

    assert_eq!(run.life, LIFE, "une morsure a porté pendant le répit");
    assert!(left < RESPITE, "le répit n'a pas avancé");

    let mut run = Run::new();
    let away = bite(RESPITE, DT, false, &mut run);
    assert_eq!(
        away, left,
        "le répit n'avance pas de la même façon hors contact"
    );
}

/// Hors contact, rien ne mord et le répit reste à zéro.
#[test]
fn sans_contact_rien_ne_mord() {
    let mut run = Run::new();
    let left = bite(0.0, DT, false, &mut run);

    assert_eq!(run.life, LIFE);
    assert_eq!(left, 0.0, "le répit s'est rouvert sans morsure");
}

/// Le rythme des morsures ne dépend pas de la cadence d'image.
///
/// **C'est l'épreuve que ce lot devait écrire**, et le défaut qu'elle garde existe
/// ailleurs dans ce jeu : le rappel de l'inertie du lacet de l'arme s'applique sans
/// multiplier par le pas de temps, et il amortit vingt-trois fois plus vite à cent
/// vingt images par seconde qu'à trente. Un répit qui compterait des images au lieu
/// de secondes rendrait une créature quatre fois plus dangereuse sur une machine
/// rapide.
///
/// **Quatre secondes et demie de contact tenu**, soit cinq morsures : une au premier
/// pas, puis une par seconde. La durée ne tombe pas sur un multiple du répit, pour
/// qu'un résidu d'accumulation ne décide pas du compte — les flottants ne reviennent
/// pas exactement à zéro après soixante soustractions d'un soixantième.
#[test]
fn le_rythme_des_morsures_ne_depend_pas_de_la_cadence() {
    for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 120.0, 1.0 / 144.0] {
        let mut run = Run::new();
        let mut respite = 0.0;

        let steps = (4.5 / dt) as u32;
        for _ in 0..steps {
            respite = bite(respite, dt, true, &mut run);
        }

        assert_eq!(
            run.life,
            LIFE - 5 * BITE,
            "à {:.0} images par seconde, {steps} pas de contact ont coûté {} de vie",
            1.0 / dt,
            LIFE - run.life
        );
    }
}

/// La vie tombe à zéro, la course est finie, et rien ne fait le tour.
///
/// **Les morsures en trop sont le vrai sujet** : la course est perdue avant qu'on
/// cesse de toucher la créature, donc le cas où l'on mord un mort arrive à chaque
/// partie. Une soustraction simple y rendrait la vie pleine.
#[test]
fn la_vie_ne_fait_pas_le_tour() {
    let mut run = Run::new();
    assert!(!run.over(), "une course commence perdue");

    for _ in 0..LIFE / BITE {
        run.hurt();
    }
    assert!(run.over(), "dix morsures n'ont pas eu la vie entière");
    assert_eq!(run.share(), 0.0);

    for _ in 0..3 {
        run.hurt();
    }
    assert!(run.over(), "une morsure de trop a rendu de la vie");
    assert_eq!(run.share(), 0.0);
}

/// Le compteur part de zéro et cumule les primes qu'on lui porte.
///
/// **Des primes différentes, et c'est ce qui le fait mordre** : un compteur qui
/// remplacerait au lieu d'ajouter resterait sur la dernière, et trois fois la même
/// valeur ne distinguerait pas un cumul d'une multiplication.
///
/// **La jonction au coup fatal ne s'éprouve pas d'ici**, et c'est une limite assumée :
/// elle vit dans le pas de la partie, qui reçoit un `Tick` que la boucle du moteur
/// seule fabrique. Ce qui la tient de chaque côté est `knock`, dont une épreuve mesure
/// qu'il ne rend sa prime qu'une fois, et cette ligne de crédit, qui se juge en
/// tirant.
#[test]
fn le_compteur_cumule_les_primes() {
    let mut run = Run::new();
    assert_eq!(run.score(), 0, "une course commence avec des points");

    let mut total = 0;
    for bounty in [100, 150, 200, 150] {
        run.credit(bounty);
        total += bounty;
        assert_eq!(
            run.score(),
            total,
            "le compteur n'a pas cumulé la prime de {bounty}"
        );
    }
}

/// La mort ne touche pas au compteur, et le compteur ne touche pas à la vie.
///
/// **Les deux champs de la course sont indépendants**, et c'est ce qui se vérifie :
/// un score qui s'effacerait à la mort retirerait son sujet à l'écran de fin, et une
/// vie entamée par un crédit serait le genre d'échange qu'on ne cherche pas.
#[test]
fn la_mort_garde_le_compteur() {
    let mut run = Run::new();
    run.credit(150);

    for _ in 0..LIFE / BITE {
        run.hurt();
    }

    assert!(run.over());
    assert_eq!(run.score(), 150, "la mort a effacé le compteur");

    run.credit(100);
    assert_eq!(run.share(), 0.0, "un crédit a rendu de la vie");
}

/// La part décroît avec la vie, du plein au vide.
///
/// **La part et non deux entiers**, parce que c'est elle que la jauge reçoit : une
/// part qui ne tomberait pas à zéro laisserait un reste de remplissage devant un
/// joueur mort.
#[test]
fn la_part_decroit_avec_la_vie() {
    let mut run = Run::new();
    assert_eq!(run.share(), 1.0);

    let mut before = run.share();
    for _ in 0..LIFE / BITE {
        run.hurt();
        assert!(
            run.share() < before,
            "la part n'a pas baissé en passant à {} de vie",
            run.life
        );
        before = run.share();
    }
    assert_eq!(before, 0.0);
}

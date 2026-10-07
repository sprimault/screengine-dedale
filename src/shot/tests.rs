// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du rayon contre le décor.
//!
//! **Des prédicats, et non une comparaison de chemins.** L'export éprouve déjà le
//! rayon du moteur contre son oracle brut, et cette égalité ne prouve rien du tir :
//! les deux chemins passent par la même géométrie, donc une formule fausse les rend
//! faux ensemble. Ce qui attrape un défaut ici est une propriété — une cloison
//! arrête, un passage laisse passer, la portée borne.
//!
//! **Elles partent de la pose réelle du joueur**, par `Player::stand`, et non d'une
//! cote composée à la main : c'est de là que le jeu tire, et une origine inventée
//! éprouverait un tir que personne ne fait.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use crate::maze::export;
use crate::maze::grid::{Grid, Settings};
use crate::player::Player;
use crate::test_support::{SEEDS, cases, maze, plain, sides, unit};

/// Ce qui sépare le centre d'une case de la cloison qui la ferme, en unités de
/// monde.
///
/// La moitié du côté intérieur d'une cellule : c'est la distance qu'un rayon tiré
/// du centre vers un côté parcourt avant de rencontrer le mur, quand il y en a un.
const TO_WALL: f32 = export::INNER / 2.0;

/// De quoi absorber la bande de contact du moteur, en unités de monde.
///
/// Un balayage rend une position *juste avant* ce qu'elle touche, donc le point
/// relevé est à un résidu près de la cloison. Un millième du côté intérieur est
/// trois ordres de grandeur au-dessus de ce résidu et reste très loin de la case
/// voisine, qui est ce que les deux épreuves doivent distinguer.
const SLACK: f32 = export::INNER / 1024.0;

/// Une cloison arrête le rayon, et sa normale s'oppose au tir.
///
/// **Les deux ensemble, parce qu'une seule ne suffit pas** : un rayon arrêté dont la
/// normale regarde ailleurs dénonce une face prise à l'envers, et le sens des sommets
/// est précisément ce qui décide de la face vue dans ce projet. La distance borne le
/// cas — ce qui arrête doit être la cloison de cette case, pas une paroi du fond.
#[test]
fn une_cloison_arrete_le_rayon() {
    let mut walled = 0;
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        for at in cases(&grid) {
            if !plain(&grid, at) {
                continue;
            }
            let player = Player::stand(&grid, &map, at);
            let cell = player.eye_cell(&map);
            if cell == 0 {
                continue;
            }

            for side in sides() {
                if !grid.has_wall(at, side) {
                    continue;
                }
                walled += 1;

                let ahead = unit(side);
                let shot = fire(&map, player.eye(), ahead, cell);
                assert!(
                    shot.blocked(),
                    "graine {seed:#x}, case {at:?} : le tir vers {side:?} traverse \
                     la cloison depuis {:?}",
                    player.eye()
                );

                let at_point = shot.impact().expect("un tir bloqué à portée a son point");
                let gone = at_point - player.eye();
                assert!(
                    gone.dot(gone).sqrt() <= TO_WALL + SLACK,
                    "graine {seed:#x}, case {at:?} : le tir vers {side:?} ne touche \
                     qu'à {} du départ, soit au-delà de la cloison",
                    gone.dot(gone).sqrt()
                );

                let hit = shot.hit.expect("la cellule de l'œil existe");
                assert!(
                    hit.normal.dot(ahead) < 0.0,
                    "graine {seed:#x}, case {at:?} : la normale {:?} ne s'oppose pas \
                     au tir vers {side:?}",
                    hit.normal
                );
            }
        }
    }

    // Sans ce compte, l'épreuve serait verte sur un décor sans aucun mur.
    assert!(
        walled > 2000,
        "seules {walled} cloisons ont été éprouvées, le corpus n'en est pas un"
    );
}

/// Un passage ouvert laisse le rayon dépasser la case.
///
/// **C'est la propriété qui discrimine**, et l'épreuve précédente ne la donne pas :
/// un rayon qui s'arrêterait à la cloison de toute case, ouverte ou non, passerait
/// la première sans que rien ne le dise. Ce qui est vérifié est donc que le tir va
/// **plus loin** que là où un mur l'aurait arrêté.
#[test]
fn un_passage_laisse_depasser_le_rayon() {
    let mut open = 0;
    let mut touched = 0;
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        for at in cases(&grid) {
            if !plain(&grid, at) {
                continue;
            }
            let player = Player::stand(&grid, &map, at);
            let cell = player.eye_cell(&map);
            if cell == 0 {
                continue;
            }

            for side in sides() {
                if grid.has_wall(at, side) {
                    continue;
                }
                // Le voisin doit être plat lui aussi : une cage de l'autre côté
                // présente ses marches au rayon, qui l'arrêtent légitimement bien
                // avant la cloison du fond.
                let Some(next) = grid.neighbour(at, side) else {
                    continue;
                };
                if !plain(&grid, next) {
                    continue;
                }
                open += 1;

                let shot = fire(&map, player.eye(), unit(side), cell);
                // Le point peut manquer si rien n'est touché à portée, ce qui est
                // encore mieux que de dépasser : c'est le passage le plus ouvert.
                if let Some(at_point) = shot.impact() {
                    touched += 1;
                    let gone = at_point - player.eye();
                    assert!(
                        gone.dot(gone).sqrt() > TO_WALL + SLACK,
                        "graine {seed:#x}, case {at:?} : le tir vers {side:?} s'arrête \
                         à {} alors que ce côté est ouvert",
                        gone.dot(gone).sqrt()
                    );
                }
            }
        }
    }

    assert!(
        open > 500,
        "seuls {open} passages ont été éprouvés, le corpus n'en est pas un"
    );
    // **Et le corpus des cas qui affirment quelque chose n'est pas le même** : un
    // tir qui ne touche rien à portée saute l'assertion, donc une portée ramenée à
    // rien rendrait l'épreuve verte et vide. Un couloir de labyrinthe finit par
    // présenter une paroi avant trente-deux unités, et la plupart le font.
    assert!(
        touched > open / 2,
        "seuls {touched} tirs sur {open} ont touché quelque chose, \
         l'épreuve n'affirme presque rien"
    );
}

/// Un tir parti de nulle part ne rencontre rien, et n'interroge pas la carte.
///
/// **Le partage est à nous, et c'est ce qu'elle fige** : le chemin Rust du moteur
/// rend l'absence aussi bien pour une cellule nulle que pour une cellule inconnue,
/// là où sa frontière C en fait deux cas distincts. Zéro veut dire hors de tout
/// volume, et un tir parti de là n'a aucun décor à rencontrer.
#[test]
fn un_tir_hors_de_tout_volume_ne_rencontre_rien() {
    let (_, map) = maze(SEEDS[0]);
    let shot = fire(&map, Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 0);

    assert!(shot.hit.is_none());
    assert!(!shot.blocked());
    assert!(shot.impact().is_none());
}

/// Le trajet demandé suit la direction visée, sur la longueur de la portée.
///
/// **Ce qu'elle attrape est dans `fire` et nulle part ailleurs** : une direction
/// inversée, une portée oubliée, un facteur appliqué deux fois. Comparer la longueur
/// à la constante qui l'a produite ne prouverait rien seul — c'est la **colinéarité**
/// qui porte l'épreuve, et elle se vérifie contre la direction visée, qui est une
/// entrée.
///
/// **Et le point touché reste dans la portée.** Un point au-delà dénoncerait une
/// fraction lue sur un autre segment que celui qui a été soumis — exactement le
/// piège que l'étape suivante doit éviter en comparant deux fractions.
#[test]
fn la_portee_borne_le_trajet() {
    let mut touched = 0;
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        for at in cases(&grid) {
            if !plain(&grid, at) {
                continue;
            }
            let player = Player::stand(&grid, &map, at);
            let cell = player.eye_cell(&map);
            if cell == 0 {
                continue;
            }

            for side in sides() {
                let ahead = unit(side);
                let shot = fire(&map, player.eye(), ahead, cell);

                // Le trajet est la direction visée, mise à l'échelle de la portée :
                // une direction inversée rendrait un produit scalaire négatif, et un
                // facteur fautif une longueur fausse.
                let span = shot.to - shot.from;
                assert!(
                    (span.dot(ahead) - RANGE).abs() <= RANGE / 1024.0,
                    "graine {seed:#x}, case {at:?} : le trajet vers {side:?} avance \
                     de {} le long de la visée, et non de {RANGE}",
                    span.dot(ahead)
                );

                if let Some(at_point) = shot.impact() {
                    touched += 1;
                    let gone = at_point - shot.from;
                    assert!(
                        gone.dot(gone).sqrt() <= RANGE,
                        "graine {seed:#x}, case {at:?} : le tir vers {side:?} touche \
                         à {}, au-delà de sa portée",
                        gone.dot(gone).sqrt()
                    );
                }
            }
        }
    }

    assert!(
        touched > 2000,
        "seuls {touched} tirs ont touché quelque chose, la borne du point \
         n'est presque jamais éprouvée"
    );
}

/// Un trajet tronqué est bloqué, et ne rend pourtant aucun point à poser.
///
/// **Le contact est fabriqué, et il doit l'être** : la troncature n'est pas
/// atteignable sur un décor que ce jeu produit — voir l'épreuve suivante —, donc la
/// seule façon d'éprouver la clause est de lui donner le contact qu'elle refuse.
/// C'est légitime : la méthode est une fonction d'un contact vers un point, et son
/// entrée est publique.
///
/// **Ce qu'elle fige est le refus, pas le contenu du point.** Que le moteur laisse
/// le point au bout du trajet demandé est une lecture de son code, pas une mesure
/// d'ici : la valeur donnée ci-dessous est celle qu'il a été vu initialiser, et
/// l'épreuve ne prétend pas la vérifier.
#[test]
fn un_trajet_tronque_ne_rend_pas_de_point() {
    let from = Vec3::new(2.0, 2.0, 1.5);
    let to = from + Vec3::new(RANGE, 0.0, 0.0);

    // Ce qu'une troncature rend : la fraction reculée, la surface et la normale
    // effacées — et le point laissé au bout du trajet.
    let shot = Shot {
        from,
        to,
        cell: 1,
        hit: Some(Hit {
            fraction: 0.5,
            normal: Vec3::ZERO,
            point: to,
            surface: 0,
            cell: 1,
            start_solid: false,
            incomplete: true,
            no_gap: false,
        }),
    };

    assert!(
        shot.blocked(),
        "un trajet tronqué arrête le tir, la réponse étant conservatrice"
    );
    assert!(
        shot.impact().is_none(),
        "le point d'un trajet tronqué est au-delà de ce qui a été examiné, \
         donc il ne se pose pas"
    );
}

/// La borne de cellules du balayage est hors d'atteinte d'un rayon axial.
///
/// **C'est une mesure, et elle explique l'épreuve précédente.** Le format d'export
/// plafonne une coordonnée de texture à `MAX_TEXEL_COORD` texels, à raison de cent
/// vingt-huit texels par unité de monde : le décor ne peut donc pas s'étendre au-delà
/// de cent vingt-huit unités, soit **trente-deux cases**. Or la borne du balayage se
/// consomme en **cellules**, et il en faut soixante-cinq dégagées pour la saturer :
/// un rayon axial n'en rencontre jamais assez, et une enfilade qui les porterait ne
/// se charge pas.
///
/// Elle part du décor **le plus long qui se charge** pour que ce soit le format qui
/// donne le chiffre, et non une constante recopiée ici.
#[test]
fn la_borne_du_balayage_est_hors_de_portee_du_decor() {
    // Une enfilade d'une case de large n'a qu'un chemin — la ligne entière —, donc
    // c'est le décor qui aligne le plus de cellules pour une étendue donnée.
    let length = 32;
    let grid = Grid::generate(Settings {
        extent: (length, 1, 1),
        seed: SEEDS[0],
        stairs: 0,
        ramps: 0,
        loops: 0,
    });
    let map = World::load(&export::world(&grid)).expect("enfilade engendrée valide");
    assert!(
        (length as usize) < screengine_play::SWEEP_CELLS,
        "une enfilade de {length} cases porte assez de cellules pour saturer \
         la borne de {}, et la troncature cesse d'être hors d'atteinte",
        screengine_play::SWEEP_CELLS
    );

    let player = Player::stand(&grid, &map, (0, 0, 0));
    let cell = player.eye_cell(&map);
    assert_ne!(cell, 0, "le joueur naît dans l'enfilade");

    // Jusqu'au-delà du bout : le rayon parcourt l'enfilade entière sans que la
    // région examinée soit bornée.
    let to = player.eye() + Vec3::new(1.0, 0.0, 0.0) * (length as f32 * export::CELL);
    let hit = map
        .pick(cell, player.eye(), to, Surfaces::Solid)
        .expect("la cellule de départ existe");
    assert!(
        !hit.incomplete,
        "le décor le plus long qui se charge tronque déjà un rayon, \
         à la fraction {}",
        hit.fraction
    );
}

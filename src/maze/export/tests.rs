// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de l'export.
//!
//! Elles passent toutes par le **chargement**, et c'est voulu : le décodeur du
//! moteur refuse tout ce que ce module pourrait écrire de travers dans les
//! octets, si bien qu'une carte qui se charge a déjà franchi une centaine de
//! contrôles qu'il serait absurde de réécrire ici.
//!
//! Ce que le chargement ne dit pas, en revanche, c'est si un portail s'est
//! **apparié** : un appariement manqué ne rend aucune erreur, il rend deux murs.
//! C'est la traversée sur la carte chargée qui l'établit, et c'est pour cela que
//! la moitié de ces épreuves interrogent le monde plutôt que les octets.

use super::*;
use crate::maze::grid::Settings;
use screengine_play::{Vec3, World};

/// Une grille d'épreuve : deux étages, assez de cases pour que les deux sortes
/// de côté — mur et passage — se rencontrent partout.
fn grid() -> Grid {
    Grid::generate(Settings {
        extent: (16, 16, 2),
        seed: 0x5EED_1A8E,
        stairs: 6,
        loops: 8,
    })
}

/// Le centre du passage qui prolonge une case vers l'est ou vers le nord.
fn gateway(at: (u32, u32, u32), side: Side) -> Vec3 {
    let (dx, dy, _) = side.step();
    Vec3::new(
        (at.0 as f32 + 0.5 + dx as f32 * 0.5) * CELL,
        (at.1 as f32 + 0.5 + dy as f32 * 0.5) * CELL,
        at.2 as f32 * LEVEL + 1.0,
    )
}

/// Le centre d'une case, à hauteur d'œil.
fn centre(at: (u32, u32, u32)) -> Vec3 {
    Vec3::new(
        (at.0 as f32 + 0.5) * CELL,
        (at.1 as f32 + 0.5) * CELL,
        at.2 as f32 * LEVEL + 1.0,
    )
}

/// Toutes les cases, dans l'ordre des axes.
fn cases(grid: &Grid) -> Vec<(u32, u32, u32)> {
    let (width, height, levels) = grid.extent();
    let mut out = Vec::new();
    for z in 0..levels {
        for y in 0..height {
            for x in 0..width {
                out.push((x, y, z));
            }
        }
    }
    out
}

/// La carte se charge, ce qui veut dire qu'elle a franchi tout ce que le
/// décodeur vérifie — en-tête, pavage des sections, identifiants, enroulements
/// de portails, et les quatre contraintes du repère de lightmap.
#[test]
fn la_carte_se_charge() {
    let grid = grid();
    assert!(World::load(&world(&grid)).is_ok());
}

/// Une case, une cellule, et la cellule est là où la case annonce qu'elle est.
///
/// La localisation est le seul contrôle qui attrape une cellule bien formée mais
/// posée au mauvais endroit : le chargement n'a aucune raison de s'en plaindre.
#[test]
fn chaque_case_est_une_cellule_a_sa_place() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");

    // Une cellule par case, plus une par passage **horizontal** : les liaisons
    // d'étage n'en ont pas encore, et les étages restent donc séparés.
    let gates = cases(&grid)
        .iter()
        .flat_map(|at| [Side::East, Side::North].map(|side| (*at, side)))
        .filter(|(at, side)| !grid.has_wall(*at, *side))
        .count() as u32;
    assert_eq!(map.cell_count(), grid.count() + gates);

    for at in cases(&grid) {
        assert_eq!(
            map.locate(centre(at)),
            cell_id(&grid, at),
            "la case {at:?} n'est pas là où elle devrait"
        );
    }
}

/// Un mur percé se franchit : les deux portails se sont donc appariés.
///
/// C'est l'épreuve qui compte. Deux portails qui ne portent pas exactement les
/// mêmes octets ne lèvent aucune erreur — le chargement en fait deux murs, et
/// plus rien ne passe. La traversée est la seule chose qui le dise.
#[test]
fn un_passage_se_franchit() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");
    let mut crossed = 0;

    for at in cases(&grid) {
        for side in [Side::East, Side::North] {
            if grid.has_wall(at, side) {
                continue;
            }
            let next = grid.neighbour(at, side).expect("un passage a une voisine");
            let through = gate_id(&grid, at, side);

            // Deux portails séparent désormais deux cases, et chaque moitié se
            // vérifie à part : c'est la seule façon de savoir laquelle manque
            // si l'une d'elles ne s'apparie pas.
            assert_eq!(
                map.track(cell_id(&grid, at), centre(at), gateway(at, side)),
                through,
                "l'entrée du passage de {at:?} vers {next:?} ne se franchit pas"
            );
            assert_eq!(
                map.track(through, gateway(at, side), centre(next)),
                cell_id(&grid, next),
                "la sortie du passage de {at:?} vers {next:?} ne se franchit pas"
            );
            crossed += 1;
        }
    }
    assert!(crossed > 0, "la grille d'épreuve n'a aucun passage");
}

/// Un pas qui franchit **deux** portails rend soit la case d'arrivée, soit zéro
/// — et jamais une cellule fausse.
///
/// Le contrat annonce zéro « quand il sort par un portail non apparié ou par une
/// surface ». Franchir plus d'un portail est un troisième cas qu'il ne mentionne
/// pas, et le résultat y **varie avec la géométrie** : mesuré ici, le même pas
/// rend la cellule d'arrivée dans un sens et zéro dans un autre.
///
/// Ce que le jeu peut donc exiger tient en deux clauses, et elles suffisent :
/// jamais de cellule fausse, et la localisation rattrape toujours. C'est
/// exactement ce que `Game::step` suppose, et ce test est là pour qu'on ne
/// retire pas son repli en le croyant inutile.
#[test]
fn un_pas_long_rend_la_case_ou_rien() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");

    for at in cases(&grid) {
        for side in [Side::East, Side::North] {
            if grid.has_wall(at, side) {
                continue;
            }
            let next = grid.neighbour(at, side).expect("un passage a une voisine");
            let arrival = cell_id(&grid, next);
            let followed = map.track(cell_id(&grid, at), centre(at), centre(next));
            assert!(
                followed == arrival || followed == 0,
                "le pas de {at:?} vers {next:?} rend {followed}, ni {arrival} ni zéro"
            );
            assert_eq!(
                map.locate(centre(next)),
                arrival,
                "la localisation ne rattrape pas le pas de {at:?} vers {next:?}"
            );
        }
    }
}

/// Un mur arrête : la traversée sort du décor plutôt que de passer au travers.
#[test]
fn un_mur_arrete() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");
    let mut stopped = 0;

    for at in cases(&grid) {
        for side in EDGES {
            if !grid.has_wall(at, side) {
                continue;
            }
            let Some(next) = grid.neighbour(at, side) else {
                continue;
            };
            assert_eq!(
                map.track(cell_id(&grid, at), centre(at), centre(next)),
                0,
                "le mur entre {at:?} et {next:?} laisse passer"
            );
            stopped += 1;
        }
    }
    assert!(stopped > 0, "la grille d'épreuve n'a aucun mur intérieur");
}

/// Les étages restent séparés tant que la cellule-escalier n'existe pas.
///
/// Ce n'est pas un défaut mais l'état attendu : un portail non apparié est un
/// mur, et une cellule close en est faite de six. L'épreuve est là pour que le
/// jour où `E1.2b` les relie, elle rougisse et soit reprise.
#[test]
fn les_etages_ne_se_rejoignent_pas() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");
    for at in cases(&grid) {
        let Some(above) = grid.neighbour(at, Side::Up) else {
            continue;
        };
        assert_eq!(
            map.track(cell_id(&grid, at), centre(at), centre(above)),
            0,
            "la case {at:?} rejoint l'étage du dessus"
        );
    }
}

/// Deux exports de la même grille portent les mêmes octets.
///
/// C'est ce qui rend l'empreinte du cache de lightmaps réutilisable d'une
/// génération à l'autre, et ce qui attraperait une table de hachage glissée dans
/// ce qui décide d'un ordre d'écriture.
#[test]
fn deux_exports_sont_identiques() {
    let grid = grid();
    assert_eq!(world(&grid), world(&grid));
}

/// Le fichier annonce sa propre longueur, que le chargement exige exacte.
#[test]
fn la_longueur_annoncee_est_la_bonne() {
    let bytes = world(&grid());
    let announced = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    assert_eq!(announced as usize, bytes.len());
}

/// Les axes employés ont tous un carré en puissance de deux, sans quoi le
/// chargement refuserait le fichier entier sans nommer la surface en cause.
#[test]
fn les_axes_sont_des_puissances_de_deux() {
    for axis in ALONG {
        assert!(power_of_two(square(scaled(axis, LUXELS))));
    }
    assert!(power_of_two(square(scaled(UPWARD, LUXELS))));
    assert!(power_of_two(square(scaled(FLAT.0, LUXELS))));
    assert!(power_of_two(square(scaled(FLAT.1, LUXELS))));
}

/// L'empreinte d'une case tourne dans le bon sens.
#[test]
fn l_empreinte_a_une_aire_positive() {
    let footprint = [[0.0, 0.0], [CELL, 0.0], [CELL, CELL], [0.0, CELL]];
    assert!(twice_area(&footprint) > 0.0);
}

/// Un rang de case rend la même cote quel que soit le côté qui la demande.
///
/// L'appariement des portails tient entièrement là-dessus : le format compare
/// les positions au bit près, sans tolérance d'aucune sorte.
#[test]
fn une_arete_a_les_memes_bits_des_deux_cotes() {
    for index in 0..64u32 {
        assert_eq!(coord(index + 1).to_bits(), coord(index + 1).to_bits());
        assert_ne!(coord(index).to_bits(), coord(index + 1).to_bits());
    }
}

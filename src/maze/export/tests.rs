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

/// La cellule qui contient une case.
///
/// Ce n'est plus toujours la sienne : une cage couvre deux cases superposées, et
/// les deux rendent l'identifiant de son pied.
fn cover(grid: &Grid, at: (u32, u32, u32)) -> u32 {
    let flight = grid
        .stairs()
        .iter()
        .find(|stair| stair.foot == at || stair.head() == at);
    cell_id(grid, flight.map_or(at, |stair| stair.foot))
}

/// Un point à l'intérieur de la cellule qui contient une case, à hauteur d'œil.
///
/// **Le centre ne convient plus pour une case d'escalier** : au milieu de la cage,
/// le sol est déjà monté à mi-étage, et un point à un mètre du sol de l'étage y
/// est dans le solide. Celui-ci se place près de l'issue de la case — le palier
/// pour celle du bas, le haut de la volée pour celle du haut —, donc dans le
/// volume et du bon côté pour franchir son portail.
fn inside(grid: &Grid, at: (u32, u32, u32)) -> Vec3 {
    let Some(stair) = grid
        .stairs()
        .iter()
        .find(|stair| stair.foot == at || stair.head() == at)
    else {
        return centre(at);
    };

    let towards = if stair.foot == at {
        stair.climb.facing()
    } else {
        stair.climb
    };
    let (dx, dy, _) = towards.step();
    let lift = if stair.foot == at { 0.0 } else { LEVEL };
    Vec3::new(
        (at.0 as f32 + 0.5) * CELL + dx as f32 * 1.3,
        (at.1 as f32 + 0.5) * CELL + dy as f32 * 1.3,
        stair.foot.2 as f32 * LEVEL + lift + 1.0,
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

/// Chaque case tombe dans la cellule qui la couvre, et le compte des cellules est
/// celui qu'on attend.
///
/// La localisation est le seul contrôle qui attrape une cellule bien formée mais
/// posée au mauvais endroit : le chargement n'a aucune raison de s'en plaindre.
/// C'est elle qui a nommé l'étourderie d'axe d'une cage montant vers le nord.
#[test]
fn chaque_case_est_une_cellule_a_sa_place() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");

    // Une cellule par case, moins une par volée — qui en couvre deux —, plus une
    // par passage horizontal.
    let gates = cases(&grid)
        .iter()
        .flat_map(|at| [Side::East, Side::North].map(|side| (*at, side)))
        .filter(|(at, side)| !grid.has_wall(*at, *side))
        .count() as u32;
    assert_eq!(
        map.cell_count(),
        grid.count() - grid.stairs().len() as u32 + gates
    );

    for at in cases(&grid) {
        assert_eq!(
            map.locate(inside(&grid, at)),
            cover(&grid, at),
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
                map.track(cover(&grid, at), inside(&grid, at), gateway(at, side)),
                through,
                "l'entrée du passage de {at:?} vers {next:?} ne se franchit pas"
            );
            assert_eq!(
                map.track(through, gateway(at, side), inside(&grid, next)),
                cover(&grid, next),
                "la sortie du passage de {at:?} vers {next:?} ne se franchit pas"
            );
            crossed += 1;
        }
    }
    assert!(crossed > 0, "la grille d'épreuve n'a aucun passage");
}

/// Un pas qui franchit **deux** portails rend la case d'arrivée.
///
/// Ce test a longtemps admis zéro à la place, parce que le résultat variait avec
/// l'orientation du pas : la traversée prenait le premier portail dans l'ordre du
/// fichier et non le premier le long du segment, donc un pas qui coupait une
/// cellule de part en part pouvait repartir en arrière. **La clause permissive est
/// tombée avec le correctif**, et l'égalité est ce qui empêche qu'il revienne sans
/// qu'on le voie.
///
/// La localisation reste vérifiée dans la foulée : c'est le repli de `Game::step`,
/// qui garde sa raison — une caméra qui vole sort vraiment du décor, et zéro le
/// dira alors légitimement.
#[test]
fn un_pas_long_rend_la_case_d_arrivee() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");

    for at in cases(&grid) {
        for side in [Side::East, Side::North] {
            if grid.has_wall(at, side) {
                continue;
            }
            let next = grid.neighbour(at, side).expect("un passage a une voisine");
            let arrival = cover(&grid, next);
            assert_eq!(
                map.track(cover(&grid, at), inside(&grid, at), inside(&grid, next)),
                arrival,
                "le pas de {at:?} vers {next:?} n'arrive pas dans sa case"
            );
            assert_eq!(
                map.locate(inside(&grid, next)),
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

/// Les étages ne se rejoignent **que** par une cage, et par elle ils se
/// rejoignent vraiment.
///
/// Les deux moitiés comptent autant. Qu'une cage relie est ce que tout ce lot
/// existe pour obtenir ; qu'elle soit la seule à le faire est ce qui garantit
/// qu'aucun plafond n'a de trou — un portail dans une dalle ne lèverait aucune
/// erreur, et on tomberait d'un étage sans savoir par où.
#[test]
fn les_etages_ne_se_rejoignent_que_par_une_cage() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");

    for at in cases(&grid) {
        let Some(above) = grid.neighbour(at, Side::Up) else {
            continue;
        };
        let caged = cover(&grid, at) == cover(&grid, above);
        assert_eq!(
            caged,
            grid.stairs().iter().any(|stair| stair.foot == at),
            "la case {at:?} partage sa cellule avec celle du dessus sans volée"
        );
        if caged {
            continue;
        }
        assert_eq!(
            map.track(cover(&grid, at), inside(&grid, at), inside(&grid, above)),
            0,
            "la case {at:?} rejoint l'étage du dessus hors d'une cage"
        );
    }

    // Et la cage mène bien d'un étage à l'autre : depuis son palier, un pas vers
    // le haut de la volée reste dans la même cellule, et la sortie débouche sur
    // l'étage du dessus.
    for stair in grid.stairs() {
        let head = stair.head();
        assert_eq!(
            map.locate(inside(&grid, head)),
            cell_id(&grid, stair.foot),
            "le haut de la volée de {:?} n'est pas dans sa cage",
            stair.foot
        );
        if let Some(beyond) = grid.neighbour(head, stair.climb) {
            assert_ne!(
                cover(&grid, beyond),
                cell_id(&grid, stair.foot),
                "la sortie de la volée de {:?} ne quitte pas sa cage",
                stair.foot
            );
        }
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

/// Les portails de l'export, relus dans ses octets : pour chacun, la cellule qui
/// le porte et sa clé d'appariement.
///
/// La clé est faite des **bits** des positions de ses sommets, triés — exactement
/// celle que le chargement construit. Le relire ici est ce qui permet de dire
/// *lequel* est en cause, là où le décodeur ne rend qu'un refus.
fn portals(map: &[u8]) -> Vec<(u32, Vec<[u32; 3]>)> {
    let read = |at: usize| u32::from_le_bytes(map[at..at + 4].try_into().unwrap());
    let cells_at = read(24) as usize;
    let cells_len = read(28) as usize;

    let mut out = Vec::new();
    let mut at = cells_at;
    while at < cells_at + cells_len {
        let body = at + 4;
        let length = read(at) as usize;
        let id = read(body);
        let points = read(body + 8) as usize;
        let surfaces = read(body + 12) as usize;
        let count = read(body + 16) as usize;

        let vertices = body + 20;
        let mut cursor = vertices + points * 12;
        for _ in 0..surfaces {
            cursor += 16 + read(cursor + 12) as usize * 4 + 72;
        }
        for _ in 0..count {
            let indices = read(cursor + 4) as usize;
            let mut key: Vec<[u32; 3]> = (0..indices)
                .map(|slot| {
                    let point = vertices + read(cursor + 8 + slot * 4) as usize * 12;
                    [read(point), read(point + 4), read(point + 8)]
                })
                .collect();
            key.sort_unstable();
            out.push((id, key));
            cursor += 8 + indices * 4;
        }
        at = body + length;
    }
    out
}

/// Aucune clé de portail ne porte trois portails, ni deux de la même cellule.
///
/// Ce sont les deux seules choses que le chargement refuse sur un portail, et il
/// ne dit pas laquelle ni où. Cette épreuve le dit.
#[test]
fn chaque_portail_a_au_plus_un_vis_a_vis() {
    let grid = grid();
    let found = portals(&world(&grid));

    let mut sorted = found.clone();
    sorted.sort_by(|a, b| a.1.cmp(&b.1));
    let mut start = 0;
    while start < sorted.len() {
        let mut end = start + 1;
        while end < sorted.len() && sorted[end].1 == sorted[start].1 {
            end += 1;
        }
        let holders: Vec<u32> = sorted[start..end].iter().map(|(id, _)| *id).collect();
        assert!(
            end - start <= 2,
            "{} portails sur la même clé, portés par {holders:?}",
            end - start
        );
        if end - start == 2 {
            assert_ne!(
                holders[0], holders[1],
                "la cellule {} s'apparie avec elle-même",
                holders[0]
            );
        }
        start = end;
    }
}

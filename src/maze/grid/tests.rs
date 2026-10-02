// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de la grille.
//!
//! Elles portent sur les propriétés et non sur les valeurs : un labyrinthe se
//! vérifie par ce qu'il garantit — une route et une seule entre deux cases, des
//! murs réciproques, un bord fermé —, jamais par une empreinte de sa forme, qui
//! changerait au moindre réglage sans rien prouver.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use std::collections::VecDeque;

/// Les réglages des épreuves : trois étages, assez de cases pour que la chasse
/// serve plusieurs fois.
fn settings() -> Settings {
    Settings {
        extent: (12, 10, 3),
        seed: 0x5EED,
        vertical_odds: 8,
    }
}

/// Toutes les cases d'une grille, dans l'ordre des axes.
fn cells(grid: &Grid) -> Vec<(u32, u32, u32)> {
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

/// Les distances de toutes les cases au départ, en nombre de passages.
fn distances(grid: &Grid) -> Vec<(u32, (u32, u32, u32))> {
    let mut depth = vec![u32::MAX; grid.walls.len()];
    let mut queue = VecDeque::new();
    let start = grid.start();
    depth[grid.offset(start.0 as i32, start.1 as i32, start.2 as i32)] = 0;
    queue.push_back(start);

    let mut out = vec![(0, start)];
    while let Some(cell) = queue.pop_front() {
        let here = depth[grid.offset(cell.0 as i32, cell.1 as i32, cell.2 as i32)];
        for side in Side::ALL {
            if grid.has_wall(cell, side) {
                continue;
            }
            let Some(next) = grid.neighbour(cell, side) else {
                continue;
            };
            let offset = grid.offset(next.0 as i32, next.1 as i32, next.2 as i32);
            if depth[offset] != u32::MAX {
                continue;
            }
            depth[offset] = here + 1;
            out.push((here + 1, next));
            queue.push_back(next);
        }
    }
    out
}

/// La même graine rend le même labyrinthe — la propriété dont tout le reste de
/// l'étape dépend.
#[test]
fn la_meme_graine_rend_le_meme_labyrinthe() {
    let first = Grid::generate(settings());
    let second = Grid::generate(settings());
    assert_eq!(first.walls, second.walls);
    assert_eq!(first.start(), second.start());
    assert_eq!(first.exit(), second.exit());
}

/// Une graine voisine rend un autre labyrinthe : le tirage sert vraiment.
#[test]
fn une_autre_graine_rend_un_autre_labyrinthe() {
    let first = Grid::generate(settings());
    let second = Grid::generate(Settings {
        seed: settings().seed + 1,
        ..settings()
    });
    assert_ne!(first.walls, second.walls);
}

/// Le creusement atteint chaque case : aucune ne garde ses six murs.
#[test]
fn toutes_les_cases_sont_atteintes() {
    let grid = Grid::generate(settings());
    for cell in cells(&grid) {
        let walls = grid.at(cell.0 as i32, cell.1 as i32, cell.2 as i32);
        assert_ne!(walls, INTACT, "la case {cell:?} n'a jamais été atteinte");
    }
}

/// Un mur percé l'est des deux côtés : sans quoi un portail s'apparierait d'un
/// seul, et le chargement en ferait deux murs sans rien dire.
#[test]
fn les_murs_sont_reciproques() {
    let grid = Grid::generate(settings());
    for cell in cells(&grid) {
        for side in Side::ALL {
            let Some(next) = grid.neighbour(cell, side) else {
                continue;
            };
            assert_eq!(
                grid.has_wall(cell, side),
                grid.has_wall(next, side.facing()),
                "le mur entre {cell:?} et {next:?} ne concorde pas"
            );
        }
    }
}

/// Entre deux cases, une route et une seule : le graphe est connexe et porte
/// exactement `cases − 1` passages, ce qui le caractérise comme un arbre.
#[test]
fn une_seule_route_entre_deux_cases() {
    let grid = Grid::generate(settings());

    let mut halves = 0;
    for cell in cells(&grid) {
        for side in Side::ALL {
            if grid.neighbour(cell, side).is_some() && !grid.has_wall(cell, side) {
                halves += 1;
            }
        }
    }
    assert_eq!(halves % 2, 0, "un passage compté une seule fois");
    assert_eq!(halves / 2, grid.count() - 1, "le compte des passages");

    let reached = distances(&grid);
    assert_eq!(
        reached.len() as u32,
        grid.count(),
        "toutes les cases sont reliées"
    );
}

/// Les faces qui donnent sur le vide restent fermées, ce qui laisse chaque
/// cellule close à l'export.
#[test]
fn le_bord_reste_ferme() {
    let grid = Grid::generate(settings());
    for cell in cells(&grid) {
        for side in Side::ALL {
            if grid.neighbour(cell, side).is_none() {
                assert!(
                    grid.has_wall(cell, side),
                    "la case {cell:?} est ouverte sur le vide par {side:?}"
                );
            }
        }
    }
}

/// Un seul étage ne perce aucun passage vertical : le frein n'a pas à voir avec
/// les bords, et une grille plate reste plate.
#[test]
fn un_seul_etage_ne_monte_pas() {
    let grid = Grid::generate(Settings {
        extent: (8, 8, 1),
        ..settings()
    });
    for cell in cells(&grid) {
        assert!(grid.has_wall(cell, Side::Up));
        assert!(grid.has_wall(cell, Side::Down));
    }
}

/// Le frein laisse passer les montées dont une case n'a pas d'autre issue : deux
/// étages d'une seule case ne se relient que verticalement.
#[test]
fn une_montee_sans_alternative_est_retenue() {
    let grid = Grid::generate(Settings {
        extent: (1, 1, 2),
        vertical_odds: 1_000,
        ..settings()
    });
    assert!(!grid.has_wall((0, 0, 0), Side::Up));
    assert!(!grid.has_wall((0, 0, 1), Side::Down));
}

/// La sortie est bien la case la plus éloignée du départ.
///
/// C'est la propriété que `farthest` annonce, et la seule qui garantisse qu'un
/// labyrinthe ne se traverse pas en trois pas. Elle ne dit rien de la distance à
/// vol d'oiseau, qui peut être de deux cases pour un parcours de cent
/// soixante-quinze passages — c'est ce que les étages font.
#[test]
fn la_sortie_est_la_case_la_plus_eloignee() {
    let grid = Grid::generate(settings());
    let reached = distances(&grid);
    let longest = reached.iter().map(|(depth, _)| *depth).max().unwrap_or(0);
    let (depth, _) = reached
        .iter()
        .find(|(_, cell)| *cell == grid.exit())
        .copied()
        .unwrap_or((0, grid.start()));
    assert_eq!(
        depth,
        longest,
        "la sortie {:?} est à {depth} passages, le maximum est {longest}",
        grid.exit()
    );
}

/// Une grille d'une seule case n'a aucun mur à percer, et ne panique pas. Sa
/// case reste intacte, seul cas où cela ne veut pas dire « jamais atteinte ».
#[test]
fn une_case_seule_est_legitime() {
    let grid = Grid::generate(Settings {
        extent: (1, 1, 1),
        ..settings()
    });
    assert_eq!(grid.count(), 1);
    assert_eq!(grid.start(), (0, 0, 0));
    assert_eq!(grid.exit(), (0, 0, 0));
    for side in Side::ALL {
        assert!(grid.has_wall((0, 0, 0), side));
    }
}

/// Les deux calculs que l'ordre des variantes porte.
#[test]
fn les_cotes_se_repondent() {
    for side in Side::ALL {
        assert_eq!(side.facing().facing(), side);
        assert_ne!(side.bit(), side.facing().bit());
        let (dx, dy, dz) = side.step();
        let (fx, fy, fz) = side.facing().step();
        assert_eq!((dx + fx, dy + fy, dz + fz), (0, 0, 0));
    }
}

/// Une dimension nulle est un réglage faux, pas une entrée à tolérer.
#[test]
#[should_panic(expected = "une grille sans case")]
fn une_dimension_nulle_panique() {
    let _ = Grid::generate(Settings {
        extent: (4, 0, 1),
        ..settings()
    });
}

// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du semis : ce qu'il écarte, ce qu'il ne répète pas, et ce qu'il
//! rejoue.
//!
//! **Le corpus décide ici, et c'est mesuré.** Débrancher le marquage des cases de
//! l'ancien placement laissait six graines vertes : il a fallu soixante-quatre
//! grilles **et** la bonne propriété pour qu'une silhouette née sur la case de départ
//! se voie. Chaque propriété tourne donc sur soixante-quatre labyrinthes, et non sur
//! les six du dépôt.
//!
//! **La distance se vérifie par un second parcours, écrit autrement.** L'épreuve qui
//! réemploierait le parcours du module serait verte sur un parcours faux — c'est la
//! forme d'angle mort que les deux oracles de balayage ont eue, tous deux passant par
//! la même formule. La relaxation ci-dessous n'a ni file ni ordre de visite, donc
//! elle ne partage rien avec ce qu'elle juge.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use std::collections::BTreeMap;

use super::*;
use crate::maze::grid::Settings;

/// Combien de labyrinthes chaque propriété éprouve.
const GRIDS: u64 = 64;

/// Un labyrinthe du gabarit du jeu, sur une graine.
fn maze(seed: u64) -> Grid {
    Grid::generate(Settings {
        extent: (16, 16, 2),
        seed,
        stairs: 6,
        ramps: 2,
        loops: 8,
    })
}

/// Le semis d'un labyrinthe, sur un générateur posé sur sa graine.
///
/// **Le générateur est explicite ici parce qu'il l'est dans le jeu** : la partie en
/// construit un seul et le passe au semis puis à la population. Ce qui devient
/// vérifiable est que deux générateurs de même graine rendent le même semis, au lieu
/// que la fonction le garantisse sans qu'on puisse le voir.
fn sown(grid: &Grid, count: usize) -> Vec<(u32, u32, u32)> {
    spots(grid, count, &mut Rng::new(grid.seed()))
}

/// La distance de parcours de chaque case au départ, par relaxation jusqu'à
/// stabilité.
///
/// **Aucune file et aucun ordre de visite**, à l'inverse du parcours en largeur du
/// module : on repasse sur toutes les cases tant qu'une distance s'améliore. C'est
/// lent et c'est sans importance — ce qu'on achète est de ne rien partager avec ce
/// qu'on juge.
fn relaxed(grid: &Grid) -> BTreeMap<(u32, u32, u32), u32> {
    let (width, height, levels) = grid.extent();
    let mut steps = BTreeMap::from([(grid.start(), 0)]);

    let mut moved = true;
    while moved {
        moved = false;
        for z in 0..levels {
            for y in 0..height {
                for x in 0..width {
                    let cell = (x, y, z);
                    let Some(&here) = steps.get(&cell) else {
                        continue;
                    };
                    for side in Side::ALL {
                        if grid.has_wall(cell, side) {
                            continue;
                        }
                        let Some(next) = grid.neighbour(cell, side) else {
                            continue;
                        };
                        if steps.get(&next).is_none_or(|&there| there > here + 1) {
                            steps.insert(next, here + 1);
                            moved = true;
                        }
                    }
                }
            }
        }
    }

    steps
}

/// Le vivier est **exactement** ce qui est assez loin du départ et hors des cages.
///
/// **L'égalité et non l'inclusion, et c'est ce qui décide.** Vérifier que les cases
/// retenues sont assez loin ne distingue pas une distance de parcours d'une distance
/// de coordonnées : celle-ci est toujours **plus petite** — chaque pas ne change
/// qu'une coordonnée d'une unité —, donc elle ne laisse passer aucune case trop
/// proche, elle écarte des cases légitimes. Un couloir sinueux qui revient près du
/// départ serait retiré du semis sans qu'une inclusion s'en aperçoive, et la moitié
/// du labyrinthe peut y passer.
///
/// L'égalité attrape les deux sens à la fois, et c'est le seul prédicat qui le fait.
#[test]
fn le_vivier_est_exactement_ce_qui_est_assez_loin() {
    for seed in 0..GRIDS {
        let grid = maze(seed);
        let cages = stairwells(&grid);
        let steps = relaxed(&grid);

        let wanted: Vec<_> = steps
            .iter()
            .filter(|(cell, steps)| **steps >= CLEARANCE && !cages.contains(cell))
            .map(|(&cell, _)| cell)
            .collect();
        let mut pool = far_enough(&grid);
        pool.sort_unstable();

        assert_eq!(pool, wanted, "graine {seed} : le vivier n'est pas le bon");
    }
}

/// Ce que le semis tire sort du vivier, et de nulle part ailleurs.
///
/// L'épreuve ci-dessus juge le vivier ; celle-ci juge le tirage, qui pourrait rendre
/// une case qu'aucune règle n'a retenue — un indice calculé à côté suffirait.
#[test]
fn le_semis_tire_dans_le_vivier() {
    for seed in 0..GRIDS {
        let grid = maze(seed);
        let pool = far_enough(&grid);

        for spot in sown(&grid, 8) {
            assert!(
                pool.contains(&spot),
                "graine {seed} : la case {spot:?} ne vient pas du vivier"
            );
        }
    }
}

/// Le tirage brasse le vivier, au lieu d'en prendre le début.
///
/// **Un semis déterministe peut être parfaitement biaisé** : prendre les premières
/// cases du vivier rejoue à l'identique sur une même graine, passe la reproductibilité
/// et fait naître tout le monde dans le même coin du labyrinthe, le vivier étant
/// construit dans l'ordre de la grille. Rien d'autre que cette propriété ne le dit.
#[test]
fn le_tirage_brasse_le_vivier() {
    let heads = (0..GRIDS).filter(|&seed| {
        let grid = maze(seed);
        sown(&grid, 3) == far_enough(&grid)[..3]
    });

    assert!(
        heads.count() < 2,
        "le semis rend le début du vivier, donc il ne brasse pas"
    );
}

/// Le semis ne rend jamais deux fois la même case.
///
/// Sans quoi deux créatures se superposent — ce que `D32` rend désormais possible en
/// marchant, mais qui à la naissance dit que le tirage remet ce qu'il a pris, et
/// qu'une case sur deux du semis est perdue.
#[test]
fn aucune_case_n_est_semee_deux_fois() {
    for seed in 0..GRIDS {
        let grid = maze(seed);
        let taken = sown(&grid, 8);

        for (rank, spot) in taken.iter().enumerate() {
            assert!(
                !taken[..rank].contains(spot),
                "graine {seed} : la case {spot:?} est semée deux fois"
            );
        }
    }
}

/// Le semis ne rend jamais la case de départ.
///
/// **Elle est le défaut historique du placement**, et elle garde son épreuve bien que
/// le dégagement l'écarte déjà : un dégagement ramené à zéro rendrait l'épreuve
/// ci-dessus vacue, et celle-ci serait alors la seule à dire qu'une créature naît
/// dans le joueur.
#[test]
fn le_depart_ne_recoit_rien() {
    for seed in 0..GRIDS {
        let grid = maze(seed);
        assert!(
            !sown(&grid, 8).contains(&grid.start()),
            "graine {seed} : la case de départ est semée, donc dans le joueur"
        );
    }
}

/// Une cage ne reçoit rien.
///
/// Les deux cases d'une volée se traversent et ne s'élisent pas : l'étage de ce qui y
/// naîtrait n'est pas décidable, et un objet posé sur des marches se relèverait à une
/// hauteur dont rien ne répond. Le corpus porte six volées par grille, donc douze
/// cases interdites sur cinq cent douze.
#[test]
fn aucune_cage_ne_recoit_rien() {
    for seed in 0..GRIDS {
        let grid = maze(seed);
        let cages = stairwells(&grid);

        for spot in sown(&grid, 8) {
            assert!(
                !cages.contains(&spot),
                "graine {seed} : la case {spot:?} est une case de cage"
            );
        }
    }
}

/// Le semis porte sur tous les étages.
///
/// **C'est ce qui éprouve que le parcours emprunte les passages verticaux.** Le
/// placement d'avant les écartait pour garder ses silhouettes en vue, ce qui bornait
/// le semis à l'étage du départ ; rien d'autre que cette propriété ne le dirait, le
/// tirage de trois cases pouvant tomber sur un seul étage par hasard. La mesure porte
/// donc sur l'ensemble des cases retenues, pas sur ce qui en est tiré.
#[test]
fn le_semis_porte_sur_tous_les_etages() {
    for seed in 0..GRIDS {
        let grid = maze(seed);
        let (_, _, levels) = grid.extent();
        let pool = far_enough(&grid);

        for level in 0..levels {
            assert!(
                pool.iter().any(|&(_, _, z)| z == level),
                "graine {seed} : l'étage {level} ne reçoit aucune case"
            );
        }
    }
}

/// Un même labyrinthe rejoue le même semis.
///
/// **C'est ce qui rend un défaut de placement instruisible** : une créature née dans
/// un mur ne se retrouve que si la graine la remet au même endroit, et c'est la
/// propriété que `D10` demande dans ses deux branches.
///
/// **Elle ne juge que le déterminisme**, et pas que la graine serve à disperser : le
/// brassage a son épreuve, et l'y mêler donnait un message qui accusait l'un pour un
/// défaut de l'autre.
#[test]
fn un_meme_labyrinthe_rejoue_le_meme_semis() {
    for seed in 0..GRIDS {
        let grid = maze(seed);
        assert_eq!(
            sown(&grid, 8),
            sown(&grid, 8),
            "graine {seed} : deux semis d'un même labyrinthe diffèrent"
        );
    }
}

/// Un labyrinthe qui n'offre pas assez de cases en rend moins, et jamais le départ.
///
/// **C'est le refus, et il remplace un repli.** Replier sur la case de départ faisait
/// naître les surnuméraires dans le joueur dès que le compte montait ; rendre moins de
/// cases laisse l'appelant en créer moins, ce que son `zip` fait de lui-même.
///
/// Deux décors, parce qu'ils ne refusent pas pour la même raison : quatre cases dont
/// aucune n'atteint le dégagement n'en rendent aucune, là où seize en rendent
/// quelques-unes mais pas les cent demandées.
#[test]
fn un_labyrinthe_trop_petit_rend_moins_que_demande() {
    let settings = |extent| Settings {
        extent,
        seed: 1,
        stairs: 0,
        ramps: 0,
        loops: 0,
    };

    let cramped = Grid::generate(settings((2, 2, 1)));
    assert!(
        sown(&cramped, 3).is_empty(),
        "quatre cases n'atteignent pas un dégagement de {CLEARANCE}, \
         donc le semis n'a rien à rendre"
    );

    let small = Grid::generate(settings((4, 4, 1)));
    let taken = sown(&small, 100);
    assert!(
        taken.len() < 100,
        "seize cases ne peuvent pas en rendre cent, et le semis en rend {}",
        taken.len()
    );
    assert!(
        !taken.contains(&small.start()),
        "le refus ne doit pas se replier sur la case de départ"
    );
}

/// Le semis donne autant de cases à chaque étage, ou tout ce qu'il peut.
///
/// **C'est la patrouille qui l'exige**, et rien avant elle ne l'exigeait : tant qu'une
/// créature pouvait monter un escalier sans qu'une ligne le sache, une répartition
/// inégale se corrigeait d'elle-même. Depuis qu'elle tient son étage, les onze d'en
/// haut et les cinq d'en bas de la mesure resteraient là où ils sont.
///
/// **Un étage peut rendre moins que sa part**, et c'est le cas prévu : le dégagement
/// autour du départ ne retire des cases qu'à l'étage du joueur. Ce qui est exigé est
/// donc sa part **ou tout son vivier**, jamais un report sur l'autre étage — qui
/// rendrait le compte total au prix de ce que l'épreuve garde.
///
/// **Soixante-quatre labyrinthes**, comme les autres propriétés de ce module : un seul
/// décor ne dirait rien d'une répartition, le vivier de chaque étage dépendant du
/// creusement.
#[test]
fn chaque_etage_recoit_sa_part() {
    /// Ce que la partie demande, et qui se divise en deux étages.
    const COUNT: usize = 16;

    for seed in 0..GRIDS {
        let grid = maze(seed);
        let (_, _, levels) = grid.extent();
        let spots = sown(&grid, COUNT);

        let pool = far_enough(&grid);
        for level in 0..levels {
            let got = spots.iter().filter(|cell| cell.2 == level).count();
            let stock = pool.iter().filter(|cell| cell.2 == level).count();
            let share =
                COUNT / levels as usize + usize::from((level as usize) < COUNT % levels as usize);

            assert_eq!(
                got,
                share.min(stock),
                "graine {seed} : l'étage {level} reçoit {got} cases pour une part de \
                 {share} et un vivier de {stock}"
            );
        }
    }
}

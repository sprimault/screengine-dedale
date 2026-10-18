// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Où les choses naissent dans le labyrinthe : un semis, pour toutes les familles.
//!
//! **Un seul tirage, et c'est lui qui rend les cases disjointes.** Deux semis
//! indépendants — un pour les créatures, un pour les ramassages — finiraient par
//! élire la même case, et une fiole naîtrait dans une créature. L'appelant demande
//! donc le **total** en une fois et découpe ce qu'il reçoit : c'est la seule partie
//! difficile du problème, et elle ne s'écrit qu'ici.
//!
//! **Le dégagement se mesure en cases parcourues, jamais en coordonnées.** Deux
//! cases voisines peuvent être séparées par un mur : elles sont alors loin l'une de
//! l'autre par les couloirs, qui est la seule distance qu'un joueur éprouve. C'est
//! un parcours en largeur qui la donne, et rien d'autre ne la donne.
//!
//! **Le parcours emprunte les passages verticaux**, là où le placement qu'il
//! remplace les écartait pour garder ses trois silhouettes en vue au lancement. C'est
//! ce qui porte le semis sur **tous** les étages, et c'est l'inverse du besoin
//! d'alors.
//!
//! **Une cage se traverse et ne s'élit pas.** Elle relie deux étages, donc l'étage
//! de ce qui y naîtrait n'est pas décidable, et un objet posé sur ses marches se
//! relèverait à une hauteur dont rien ne répond.
//!
//! **Et il rend moins de cases que demandé plutôt que de replier.** Un repli sur la
//! case de départ fait naître les surnuméraires dans le joueur dès que le compte
//! monte : ce n'est pas un repli, c'est une règle de jeu violée. Le compte effectif
//! est donc ce que le labyrinthe offre.

use std::collections::VecDeque;

use crate::maze::grid::{Grid, Side};
use crate::rng::Rng;

#[cfg(test)]
mod tests;

/// Le dégagement minimal autour du départ, en cases parcourues.
///
/// **Six, et la cote se lit contre la portée de détection** : moins que celle-ci et
/// la partie commence par une attaque, ce qu'aucune épreuve ne dira. Une case vaut
/// quatre unités de pas — trois de cellule et une de passage —, donc six cases font
/// vingt-quatre unités, au-delà de toute portée qu'une créature puisse vouloir.
///
/// **Et au-delà du brouillard, de deux unités seulement** : `scene::FOG_END` ferme la
/// vue à vingt-deux, donc une créature semée au plus près du départ y est déjà noyée.
/// La marge est mince, et c'est la cote à reprendre si le brouillard recule — une
/// créature visible du départ est ce que ce dégagement existe pour empêcher.
///
/// **À rejuger quand la détection existera**, et dans ce sens-là : c'est la portée
/// qui se règle contre le brouillard, et ce dégagement qui se règle contre elle.
const CLEARANCE: u32 = 6;

/// Les cases où semer, deux à deux distinctes et loin du départ.
///
/// Rend `count` cases, ou moins si le labyrinthe n'en offre pas assez — un petit
/// décor, ou un dégagement qui ne laisse rien. L'ordre est celui du tirage, donc un
/// appelant qui sert plusieurs familles peut découper la tranche de chacune sans
/// qu'aucune case se retrouve dans deux.
///
/// **Le générateur vient de l'appelant et ne se fabrique pas ici**, parce que ce qui
/// se pose dans le labyrinthe tire aussi autre chose — une résistance, un cap. Deux
/// générateurs posés sur la même graine rendraient deux fois la même suite, et cette
/// coïncidence fait rouvrir la question à chaque relecture. Un seul endroit connaît
/// donc la graine, et c'est celui qui construit la partie.
pub fn spots(grid: &Grid, count: usize, rng: &mut Rng) -> Vec<(u32, u32, u32)> {
    let mut pool = far_enough(grid);

    // Un mélange partiel plutôt qu'un tirage avec rejet : il tire sans remise par
    // construction, et ne brasse que ce qu'on emporte — trois cases sur cinq cents
    // ne demandent pas de mélanger cinq cents.
    let taken = count.min(pool.len());
    for slot in 0..taken {
        let pick = slot + rng.below((pool.len() - slot) as u32) as usize;
        pool.swap(slot, pick);
    }
    pool.truncate(taken);
    pool
}

/// Les cases assez loin du départ pour y semer, dans l'ordre de la grille.
///
/// **L'ordre est celui de la grille et non celui du parcours**, pour que le semis ne
/// dépende pas de la façon dont le parcours a visité : deux implémentations de file
/// rendraient alors deux semis pour une même graine.
fn far_enough(grid: &Grid) -> Vec<(u32, u32, u32)> {
    let reach = walked(grid);
    let (width, height, levels) = grid.extent();
    let stairwells = stairwells(grid);

    let mut pool = Vec::new();
    for z in 0..levels {
        for y in 0..height {
            for x in 0..width {
                let cell = (x, y, z);
                let far = reach[flat(cell, grid)].is_some_and(|steps| steps >= CLEARANCE);
                if far && !stairwells.contains(&cell) {
                    pool.push(cell);
                }
            }
        }
    }
    pool
}

/// La distance de parcours du départ à chaque case, ou rien si elle est isolée.
///
/// Une case sans distance ne se sème pas, et le cas ne vient pas du creusement, qui
/// rend un labyrinthe d'un seul tenant : il vient de ce que la clause doit tenir
/// seule si un jour une case se retrouvait hors du graphe.
fn walked(grid: &Grid) -> Vec<Option<u32>> {
    let (width, height, levels) = grid.extent();
    let mut reach = vec![None; (width * height * levels) as usize];

    let start = grid.start();
    reach[flat(start, grid)] = Some(0);
    // La distance voyage dans la file plutôt que de se relire dans le tableau :
    // la relire demanderait de déballer une valeur qu'on vient d'y poser.
    let mut queue = VecDeque::from([(start, 0)]);

    while let Some((at, steps)) = queue.pop_front() {
        for side in Side::ALL {
            if grid.has_wall(at, side) {
                continue;
            }
            let Some(next) = grid.neighbour(at, side) else {
                continue;
            };
            let slot = flat(next, grid);
            if reach[slot].is_none() {
                reach[slot] = Some(steps + 1);
                queue.push_back((next, steps + 1));
            }
        }
    }

    reach
}

/// Les deux cases qu'occupe chaque volée.
fn stairwells(grid: &Grid) -> Vec<(u32, u32, u32)> {
    grid.stairs()
        .iter()
        .flat_map(|stair| [stair.foot, stair.head()])
        .collect()
}

/// L'indice à plat d'une case, dans l'ordre de la grille.
fn flat(cell: (u32, u32, u32), grid: &Grid) -> usize {
    let (width, height, _) = grid.extent();
    ((cell.2 * height + cell.1) * width + cell.0) as usize
}

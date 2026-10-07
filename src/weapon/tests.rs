// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de l'arme en main.
//!
//! Ce qui se regarde n'est pas ici : la taille à l'écran, la position dans le
//! cadre et l'amplitude du balancement se règlent par `make run`, et aucun test ne
//! dira qu'une arme est trop grosse. Ce qui s'éprouve est l'arithmétique de son
//! cycle, qui est une fonction d'entrées vers des sorties.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;

/// La colonne de la bouche du canon dans la planche, en texels.
///
/// **C'est le texel opaque le plus haut**, et c'est ce qui rend le repère exact plutôt
/// qu'estimé à l'œil sur une capture : le revolver pointe vers le haut, donc rien du
/// dessin ne le dépasse. À égalité de hauteur, la moyenne des colonnes — la bouche est
/// large de quelques texels.
fn muzzle(sheet: &Texture) -> f32 {
    let side = SIDE as u32;
    for v in 0..side {
        let row: Vec<u32> = (0..side)
            .filter(|u| sheet.texel(0, *u as i32, v as i32) >> 24 != 0)
            .collect();
        if !row.is_empty() {
            return row.iter().sum::<u32>() as f32 / row.len() as f32;
        }
    }
    unreachable!("la planche de l'arme n'est pas vide")
}

/// Le canon tombe dans l'axe du regard, là où le réticule marque le centre.
///
/// **Sans elle, l'alignement se perd en silence** : il tient à deux choses qui ne se
/// voient pas ensemble — le décalage latéral de l'arme, et l'endroit où la bouche est
/// dessinée dans sa planche. Une planche refaite d'un geste un peu différent décalerait
/// la visée sans qu'aucune autre épreuve ne bouge, et le symptôme serait qu'on tire à
/// côté de ce qu'on pointe.
///
/// **La tolérance est de deux texels de planche**, soit moins d'un pixel à la résolution
/// interne : c'est le grain du dessin, et viser plus fin n'aurait pas de sens.
#[test]
fn le_canon_tombe_dans_l_axe_du_regard() {
    let weapon = Weapon::new(0.0).expect("planche du dépôt valide");

    // La colonne, ramenée en unités de monde depuis le centre du quadrilatère : les
    // coordonnées de texture vont de zéro à `SIDE` sur une largeur de deux `EXTENT`.
    let from_centre = EXTENT.0 * (2.0 * muzzle(&weapon.rest) / SIDE - 1.0);
    let aside = OFFSET.0 + from_centre;

    // Deux texels de planche, convertis dans la même unité.
    let slack = EXTENT.0 * 2.0 * 2.0 / SIDE;
    assert!(
        aside.abs() <= slack,
        "la bouche du canon est à {aside} de l'axe du regard, soit {} texels : \
         le réticule ne désigne pas ce que l'arme pointe",
        aside * SIDE / (2.0 * EXTENT.0)
    );
}

/// La phase avance avec la distance, et pas avec le temps.
///
/// **C'est l'instrument de mesure du déplacement**, et c'est ce qui en fait une
/// épreuve plutôt qu'un réglage : la distance arrive mesurée après filtrage, donc
/// des mains qui balancent sans qu'on avance dénoncent un déplacement appliqué
/// avant la collision. Un cycle piloté par l'horloge perdrait ce service.
#[test]
fn la_phase_suit_la_distance() {
    let mut weapon = Weapon::new(0.0).expect("planche du dépôt valide");
    assert_eq!(weapon.stride, 0.0);

    weapon.advance(0.0, 0.0, 0.0);
    assert_eq!(weapon.stride, 0.0, "à l'arrêt, le pas ne défile pas");

    weapon.advance(STRIDE, 0.0, 0.0);
    assert_eq!(weapon.stride, 1.0, "un pas complet vaut un tour");

    weapon.advance(STRIDE / 2.0, 0.0, 0.0);
    assert_eq!(weapon.stride, 1.5);
}

/// L'inertie du lacet revient à zéro quand on cesse de tourner.
///
/// **Sans quoi l'arme resterait décalée** : le rappel est ce qui la ramène dans
/// l'axe, et un facteur trop grand la ferait partir sans retour. L'épreuve fige la
/// convergence, pas la valeur du facteur.
#[test]
fn l_inertie_du_lacet_s_amortit() {
    let mut weapon = Weapon::new(0.0).expect("planche du dépôt valide");

    // Un quart de tour d'un coup : le retard naît.
    weapon.advance(0.0, 1.0, 0.0);
    let born = weapon.drag;
    assert!(born > 0.0, "tourner doit décaler l'arme, et non la laisser");

    // Puis plus rien ne tourne : il doit décroître, et vers zéro.
    for _ in 0..64 {
        weapon.advance(0.0, 1.0, 0.0);
    }
    assert!(
        weapon.drag.abs() < born / 1000.0,
        "le retard vaut encore {} après soixante-quatre pas",
        weapon.drag
    );
}

/// La pose de tir se montre au coup, et revient au repos d'elle-même.
///
/// **L'éclair se compte en secondes, pas en images** : à une cadence deux fois plus
/// haute il doit durer autant de temps et non deux fois moins. L'épreuve le vérifie
/// en deux pas inégaux, ce qu'aucune cadence fixe ne dirait.
#[test]
fn la_pose_de_tir_s_eteint_d_elle_meme() {
    let mut weapon = Weapon::new(0.0).expect("planche du dépôt valide");
    assert_eq!(weapon.flash, 0.0, "l'arme naît au repos");

    weapon.shoot();
    assert_eq!(weapon.flash, FLASH);

    // Deux pas qui, ensemble, n'épuisent pas l'éclair.
    weapon.advance(0.0, 0.0, FLASH / 4.0);
    weapon.advance(0.0, 0.0, FLASH / 4.0);
    assert!(weapon.flash > 0.0, "l'éclair s'éteint trop tôt");

    // Et un pas long, qui le dépasse : il s'arrête à zéro, jamais en deçà.
    weapon.advance(0.0, 0.0, FLASH);
    assert_eq!(weapon.flash, 0.0);
}

/// Tirer pendant l'éclair le relance, il ne s'accumule pas.
///
/// **Une arme ne tire pas deux fois en deux images**, et l'addition donnerait une
/// pose qui reste figée en avant sous un appui répété — ce qui se verrait comme un
/// défaut d'animation là où c'est une addition de trop.
#[test]
fn tirer_pendant_l_eclair_le_relance() {
    let mut weapon = Weapon::new(0.0).expect("planche du dépôt valide");

    weapon.shoot();
    weapon.advance(0.0, 0.0, FLASH / 2.0);
    weapon.shoot();
    assert_eq!(weapon.flash, FLASH);
}

/// Le lacet de départ ne produit aucun décalage.
///
/// **Le piège est de l'oublier** : le retard se mesure sur la différence des
/// lacets, donc une arme née avec un lacet nul devant une caméra tournée naîtrait
/// décalée, et reviendrait dans l'axe en un quart de seconde — visible, et
/// inexplicable à l'écran.
#[test]
fn l_arme_nait_dans_l_axe() {
    let mut weapon = Weapon::new(2.5).expect("planche du dépôt valide");
    weapon.advance(0.0, 2.5, 0.0);
    assert_eq!(weapon.drag, 0.0);
}

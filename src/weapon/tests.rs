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

/// Le balancement revient au neutre quand on cesse de marcher.
///
/// **L'épreuve ci-dessus ne suffisait pas, et c'est ce qui a laissé passer le défaut** :
/// elle construit une arme neuve, donc une phase nulle dont le sinus l'est aussi. Elle
/// mesure l'arme **au neutre**, jamais après une marche — et après une marche, l'arme
/// restait écartée du centre jusqu'à six fois l'alignement du canon, parce que la phase
/// ne bouge plus à l'arrêt et que rien n'éteignait son amplitude.
///
/// **La phase, elle, ne doit pas revenir** : elle est l'instrument qui dénonce un
/// déplacement appliqué avant la collision, donc la ramener pour servir le cadrage
/// reviendrait à casser une mesure pour arranger une image. C'est ce que le second point
/// vérifie.
#[test]
fn le_balancement_revient_au_neutre_a_l_arret() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let mut weapon = Weapon::new(0.0).expect("planche du dépôt valide");

    // Trois quarts de seconde de marche : l'amplitude est déployée, et la phase
    // s'arrête à trois quarts de tour, là où le sinus vaut l'unité. **Une seconde
    // ronde tomberait sur un tour entier**, donc sur un sinus nul, et l'épreuve ne
    // mesurerait rien — c'est la garde ci-dessous qui l'a dit.
    for _ in 0..45 {
        weapon.advance(STRIDE * DT, 0.0, DT);
    }
    assert!(
        weapon.sway > 0.9,
        "après une seconde de marche, l'amplitude ne vaut que {}",
        weapon.sway
    );
    let walked = weapon.stride;
    assert!(
        (walked * core::f32::consts::TAU).sin().abs() > 0.1,
        "la marche s'arrête sur une phase dont le sinus est négligeable, \
         donc l'épreuve ne verrait pas l'écart qu'elle cherche"
    );

    // Une demi-seconde d'arrêt.
    for _ in 0..30 {
        weapon.advance(0.0, 0.0, DT);
    }

    // **Le seuil n'est pas choisi, il est dérivé de ce qui compte** : l'écart latéral
    // que le balancement résiduel peut encore produire vaut `SWAY.0 × sway`, et il doit
    // rester sous la tolérance d'alignement du canon — deux texels de planche, celle de
    // l'épreuve ci-dessus. Un chiffre rond dirait seulement que l'amplitude est petite,
    // pas qu'elle est assez petite pour que le réticule désigne ce que l'arme pointe.
    let slack = EXTENT.0 * 2.0 * 2.0 / SIDE;
    assert!(
        SWAY.0 * weapon.sway <= slack,
        "à l'arrêt, le balancement laisse le canon à {} de l'axe, au-delà des \
         {slack} que l'alignement tolère",
        SWAY.0 * weapon.sway
    );
    assert_eq!(
        weapon.stride, walked,
        "la phase a bougé à l'arrêt, donc elle ne dénonce plus un déplacement \
         appliqué avant la collision"
    );
}

/// L'amplitude du balancement ne dépend pas de la cadence.
///
/// **Le rappel du lacet, lui, en dépend** — il s'applique sans pas de temps —, et c'est
/// un défaut de ce fichier que ce lot n'étend pas. Une demi-seconde doit éteindre le
/// balancement autant à trente images par seconde qu'à cent vingt.
#[test]
fn l_amplitude_ne_depend_pas_de_la_cadence() {
    let settled = |dt: f32| {
        let mut weapon = Weapon::new(0.0).expect("planche du dépôt valide");
        for _ in 0..(1.0 / dt) as usize {
            weapon.advance(STRIDE * dt, 0.0, dt);
        }
        for _ in 0..(0.25 / dt) as usize {
            weapon.advance(0.0, 0.0, dt);
        }
        weapon.sway
    };

    let (slow, quick) = (settled(1.0 / 30.0), settled(1.0 / 120.0));
    assert!(
        (slow - quick).abs() <= 0.05,
        "l'amplitude retombe à {slow} à trente images par seconde et à {quick} \
         à cent vingt"
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

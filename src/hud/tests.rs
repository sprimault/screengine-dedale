// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du facteur de l'interface.
//!
//! **C'est la seule arithmétique de ce module**, le reste étant du dessin qui se
//! regarde : qu'une jauge soit lisible ou qu'un compteur domine la vue se juge à
//! l'écran, mais le facteur qu'une hauteur de tampon donne est une fonction d'entrées
//! vers des sorties, et chaque élément du HUD en dépend — le réticule excepté, qui
//! garde ses cotes en pixels.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;

/// À la hauteur d'ouverture, le facteur vaut un et les cotes réglées à l'œil valent
/// telles quelles.
///
/// **C'est l'ancrage de tout le reste** : les constantes du HUD sont des cotes à cette
/// hauteur, donc un facteur qui ne vaudrait pas un ici les décalerait toutes sans que
/// rien ne le dise.
#[test]
fn l_ouverture_donne_le_facteur_un() {
    assert_eq!(scale(resolution::BASE.1), 1);
}

/// Une hauteur double donne deux, une triple donne trois.
#[test]
fn le_facteur_suit_les_multiples_de_l_ouverture() {
    assert_eq!(scale(2 * resolution::BASE.1), 2);
    assert_eq!(scale(3 * resolution::BASE.1), 3);
}

/// Entre deux crans, le facteur arrondit au plus proche.
///
/// **C'est la clause qui distingue l'arrondi de la troncature**, et elle se mesure sur
/// 540 : une fenêtre entre deux crans garderait sinon les cotes de l'ouverture sur une
/// image une fois et demie plus haute, et l'interface y paraîtrait un tiers trop fine.
/// La moitié haute passe au cran suivant, la moitié basse reste.
#[test]
fn le_facteur_arrondit_au_plus_proche() {
    assert_eq!(scale(540), 2, "540 est la moitié, elle monte");
    assert_eq!(scale(539), 1, "539 est sous la moitié, elle reste");
    assert_eq!(scale(900), 3);
    assert_eq!(scale(899), 2);
}

/// Sous l'ouverture, le facteur ne descend jamais sous un.
///
/// **Zéro est ce qui rendrait tout invisible** : les cotes du HUD sont des produits par
/// ce facteur, donc un zéro donnerait une jauge sans épaisseur et un texte sans
/// hauteur — et le tampon ne sait pas refuser ce qui ne couvre aucun pixel. Le cas
/// arrive : le plancher de résolution est à 180, soit la moitié de l'ouverture.
#[test]
fn le_facteur_ne_tombe_jamais_a_zero() {
    for height in [0, 1, resolution::FLOOR.1, 179, 180, 359] {
        assert!(
            scale(height) >= 1,
            "la hauteur {height} donne un facteur nul"
        );
    }
}

/// Sur la résolution qu'un grand écran donne, le facteur reste celui qu'on veut.
///
/// **C'est le seul endroit où les deux politiques se rencontrent**, et la cote vient de
/// l'autre : un écran de 3840 par 2160 rend une image de 1080 de haut, donc des cotes
/// d'interface au triple — vingt et un pixels d'épaisseur pour la jauge, qui est la
/// part de l'image qu'elle occupe à l'ouverture. Un grain changé là-bas déplacerait
/// cette valeur, et c'est bien ce qu'on veut voir.
#[test]
fn un_grand_ecran_garde_la_part_de_l_interface() {
    let (_, height) = resolution::fit((3840, 2160));
    assert_eq!(scale(height), 3);
}

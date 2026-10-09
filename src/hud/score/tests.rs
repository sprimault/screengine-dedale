// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du compteur : sa largeur fixe, son ancrage, et ce qu'il refuse.
//!
//! **Ce qui s'éprouve est l'arithmétique du cadrage**, et rien de l'aspect : qu'un
//! compteur de seize pixels se lise sans dominer la vue se juge à l'écran, mais qu'il
//! occupe le coin bas droit quelle que soit la résolution est une fonction d'entrées
//! vers des sorties.
//!
//! **Rien ici ne suppose où l'encre d'un chiffre commence dans sa case.** La planche
//! ne le promet pas — l'espacement entre caractères y est dessiné, donc la dernière
//! colonne encrée s'arrête une à trois colonnes avant l'avance —, si bien que la
//! largeur se mesure sur la chaîne et l'ancrage par comparaison de deux tampons. Une
//! épreuve qui aurait lu l'empreinte au pixel près aurait été juste sur les chiffres
//! d'aujourd'hui et fausse à la première planche recuite.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use crate::test_support::Canvas;

/// Un tampon noir, et le compteur dessiné dedans à ce score.
fn drawn(width: u32, height: u32, score: u32) -> Canvas {
    let glyphs = Glyphs::new();
    let mut canvas = Canvas::new(width, height);
    draw(&mut canvas.output(), &glyphs, score);
    canvas
}

/// Les distances du coin bas droit du tampon à chaque pixel encré.
///
/// **Prises au coin et non à l'origine**, parce que c'est ce dont le compteur dépend :
/// deux tampons de tailles différentes rendent les mêmes distances quand l'ancrage est
/// juste, et des distances décalées de l'écart des cotes quand une constante a pris la
/// place du tampon.
///
/// **L'encre seule, ni la plaque ni son liseré** : ceux-ci sont des rectangles pleins
/// dont le bord ne dit rien des chiffres, et c'est le dessin des chiffres qu'on suit
/// d'un tampon à l'autre.
fn corner(canvas: &Canvas) -> Vec<(u32, u32)> {
    let mut found = Vec::new();
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            if canvas.pixel(x, y) == glyph::INK {
                found.push((canvas.width() - x, canvas.height() - y));
            }
        }
    }
    found
}

/// Le compteur garde la même largeur quel que soit le score.
///
/// **C'est la propriété des rangs de tête, et la seule** : sans eux, un compteur cadré
/// à droite se décale d'un cran entier au passage à l'ordre de grandeur suivant, et le
/// dessin saute à l'image même où un démon tombe. Les cinq franchissements sont
/// éprouvés, parce que le défaut ne se voit qu'à eux.
#[test]
fn le_compteur_garde_sa_largeur() {
    let glyphs = Glyphs::new();
    let reference = glyphs.width(&digits(0), SCALE);

    for score in [0, 7, 90, 100, 999, 1_000, 54_321, 99_999] {
        let text = digits(score);
        assert_eq!(
            text.chars().count(),
            DIGITS,
            "le compteur à {score} s'écrit « {text} », qui n'a pas {DIGITS} rangs"
        );
        assert_eq!(
            glyphs.width(&text, SCALE),
            reference,
            "le compteur à {score} n'occupe pas la largeur du compteur à zéro"
        );
    }
}

/// Un score plus grand que les rangs prévus s'étend plutôt que de se tronquer.
///
/// **Le bon échec est vers la gauche** : un compteur tronqué afficherait un nombre
/// faux sans rien signaler, là qu'un nombre plus large reste exact et déborde dans un
/// coin vide. Rien ne le garde dans le code — c'est une propriété du formatage —, et
/// c'est pour cela qu'elle s'éprouve.
///
/// **Les deux scores finissent par le même chiffre**, pour que le bord droit du dessin
/// se compare : un dernier rang différent déplacerait la dernière colonne encrée sans
/// que le cadrage ait bougé.
#[test]
fn un_score_hors_des_rangs_ne_se_tronque_pas() {
    assert_eq!(digits(100_000), "100000");

    let five = corner(&drawn(320, 180, 99_999));
    let six = corner(&drawn(320, 180, 199_999));

    assert!(
        six.len() > five.len(),
        "un score à six rangs n'a pas plus d'encre qu'un score à cinq"
    );
    assert_eq!(
        six.iter().map(|&(x, _)| x).min(),
        five.iter().map(|&(x, _)| x).min(),
        "le compteur a quitté son bord droit en gagnant un rang"
    );
}

/// Le compteur se pose au coin bas droit, et son coin vient du tampon.
///
/// **Trois tampons de proportions différentes, et c'est ce qui le fait mordre** : une
/// cote écrite en constante tombe juste le jour où on l'écrit et se décolle du coin au
/// premier changement de résolution, sans que rien ne le dise.
///
/// **La marge se vérifie par le bas**, et pas à l'égalité : l'encre est en retrait de
/// l'air de la plaque, et la dernière colonne encrée d'un chiffre s'arrête avant son
/// avance. Ce que l'inégalité attrape est un compteur collé au bord, et l'égalité des
/// trois empreintes attrape tout décalage.
#[test]
fn le_coin_du_compteur_vient_du_tampon() {
    let reference = corner(&drawn(320, 180, 1_234));
    assert!(
        !reference.is_empty(),
        "le compteur n'a rien dessiné sur un tampon à sa taille"
    );

    for (width, height) in [(200, 120), (160, 100), (480, 320)] {
        assert_eq!(
            corner(&drawn(width, height, 1_234)),
            reference,
            "sur un tampon de {width}×{height}, le compteur n'est pas au même coin"
        );
    }

    let right = reference
        .iter()
        .map(|&(x, _)| x)
        .min()
        .expect("une colonne encrée");
    let bottom = reference
        .iter()
        .map(|&(_, y)| y)
        .min()
        .expect("une ligne encrée");
    assert!(
        right > INSET && bottom > INSET,
        "le compteur touche son bord : {right} du droit et {bottom} du bas, \
         pour une marge de {INSET}"
    );
}

/// Un tampon trop petit ne reçoit rien, plutôt que de déborder.
///
/// **Les deux cotes se calculent par soustraction**, donc sur un entier non signé un
/// tampon plus petit que le compteur ne passe pas sous zéro : il déborde par le haut,
/// et le texte partirait à l'autre bout de la ligne — ou nulle part, le tampon sautant
/// ce qui le dépasse, ce qui est pire parce qu'alors rien ne le dit.
///
/// **Chaque refus se vérifie avec le premier cas qui passe** : sans lui, un refus de
/// tout passerait. Et les deux cotes minimales se cherchent par balayage plutôt que de
/// se recopier : elles dépendent de l'avance des chiffres et du nombre de rangs, donc
/// un nombre écrit ici serait juste aujourd'hui et faux au premier réglage.
#[test]
fn un_tampon_trop_petit_ne_recoit_rien() {
    let painted = |width: u32, height: u32| {
        drawn(width, height, 1_234)
            .pixels()
            .iter()
            .any(|byte| *byte != 0)
    };

    let narrow = (1..320)
        .find(|&width| painted(width, 180))
        .expect("une largeur sous trois cent vingt porte le compteur");
    assert!(
        !painted(narrow - 1, 180),
        "un tampon de {} de large a reçu un compteur qui n'y tient pas",
        narrow - 1
    );

    let short = (1..180)
        .find(|&height| painted(320, height))
        .expect("une hauteur sous cent quatre-vingts porte le compteur");
    assert!(
        !painted(320, short - 1),
        "un tampon de {} de haut a reçu un compteur qui n'y tient pas",
        short - 1
    );
}

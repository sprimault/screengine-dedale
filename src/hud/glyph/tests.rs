// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du texte à l'écran.

use super::*;
use crate::test_support::Canvas;

/// Une teinte franche, qu'aucun fond de tampon ne porte par accident.
const INK: [u8; 4] = [0xFF, 0xD0, 0x40, 0xFF];

/// Les pixels peints d'un tampon, en coordonnées, dans l'ordre de balayage.
fn painted(canvas: &Canvas) -> Vec<(u32, u32)> {
    (0..canvas.height())
        .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| canvas.pixel(x, y) == INK)
        .collect()
}

/// Aucune encre ne déborde de l'avance que la table annonce.
///
/// **C'est ce qui rattache la table à la planche**, et elle en est une seconde source :
/// une avance trop courte ferait chevaucher la lettre suivante, et c'est le sens qui
/// fait mal. L'autre sens — une avance trop longue — ne se mesure pas d'ici, la
/// colonne vide d'un glyphe étant indiscernable de son espacement.
///
/// **La case se retrouve depuis la position du texel, et non par la formule du
/// dessin.** Refaire le même calcul laisserait une origine fausse passer des deux
/// côtés ; ici le balayage part de la planche entière et déduit le rang, ce qui est
/// l'inverse du chemin que [`Glyphs::draw`] emprunte.
#[test]
fn aucune_encre_ne_deborde_de_son_avance() {
    let glyphs = Glyphs::new();
    let side = CELL * COLUMNS;

    let mut vus = 0;
    for y in 0..side {
        for x in 0..side {
            if glyphs.sheet.texel(0, x as i32, y as i32) >> ALPHA == 0 {
                continue;
            }
            vus += 1;
            let rank = (y / CELL * COLUMNS + x / CELL) as usize;
            let column = x % CELL;
            assert!(
                column < ADVANCE[rank] as u32,
                "le glyphe {:?} encre sa colonne {column} pour une avance de {}",
                char::from_u32(FIRST + rank as u32),
                ADVANCE[rank]
            );
        }
    }
    assert_eq!(vus, 775, "la planche n'a plus le même dessin");
}

/// Les dix chiffres avancent de la même largeur.
///
/// **C'est ce qui tient un score immobile** : un compteur dont les chiffres
/// n'occupent pas la même place se déplace à chaque unité, et le regard le suit au
/// lieu de le lire.
#[test]
fn les_dix_chiffres_avancent_pareil() {
    let glyphs = Glyphs::new();
    let largeurs: Vec<u32> = ('0'..='9')
        .map(|c| glyphs.width(&c.to_string(), 1))
        .collect();
    assert!(
        largeurs.windows(2).all(|pair| pair[0] == pair[1]),
        "les chiffres n'ont pas tous la même avance : {largeurs:?}"
    );
}

/// Une minuscule se replie sur sa majuscule.
#[test]
fn une_minuscule_se_replie_sur_sa_majuscule() {
    assert_eq!(rank('a'), rank('A'));
    assert_eq!(rank('z'), rank('Z'));
}

/// Un caractère hors du répertoire ne dessine rien et n'avance pas.
///
/// **Le mot se raccourcit, et c'est le but** : une espace à la place d'un accent
/// passerait pour une césure voulue, là où un mot amputé se remarque.
#[test]
fn un_caractere_hors_repertoire_n_avance_pas() {
    let glyphs = Glyphs::new();
    assert_eq!(glyphs.width("É", 1), 0);
    assert_eq!(glyphs.width("MÉTÉO", 1), glyphs.width("MTO", 1));

    let mut canvas = Canvas::new(32, 16);
    glyphs.draw(&mut canvas.output(), (0, 0), "É", 1, INK);
    assert!(
        painted(&canvas).is_empty(),
        "un accent a dessiné quelque chose"
    );
}

/// Ce qui est peint tient dans la largeur annoncée, et sur la hauteur d'une case.
///
/// **C'est la propriété sur laquelle un appelant centre son texte** : `width` doit
/// majorer ce qui sera réellement posé, sinon un libellé centré déborde d'un côté.
/// L'épreuve ne compare aucune forme — elle borne, et une bordure la falsifie.
#[test]
fn le_texte_tient_dans_la_largeur_annoncee() {
    let glyphs = Glyphs::new();
    let mut canvas = Canvas::new(128, 32);
    let (x, y) = (5, 3);
    glyphs.draw(&mut canvas.output(), (x, y), "SCORE", 1, INK);

    let peints = painted(&canvas);
    assert!(!peints.is_empty(), "rien n'a été peint");

    let largeur = glyphs.width("SCORE", 1);
    for (px, py) in peints {
        assert!(
            (x..x + largeur).contains(&px) && (y..y + CELL).contains(&py),
            "le pixel ({px} {py}) sort de la boîte annoncée, large de {largeur}"
        );
    }
}

/// L'échelle réplique les pixels : le quadruple à l'échelle deux.
#[test]
fn l_echelle_replique_les_pixels() {
    let glyphs = Glyphs::new();

    let mut simple = Canvas::new(128, 32);
    glyphs.draw(&mut simple.output(), (0, 0), "SCORE", 1, INK);

    let mut double = Canvas::new(128, 32);
    glyphs.draw(&mut double.output(), (0, 0), "SCORE", 2, INK);

    assert_eq!(painted(&double).len(), painted(&simple).len() * 4);
    assert_eq!(glyphs.width("SCORE", 2), glyphs.width("SCORE", 1) * 2);
}

/// Un texte posé au bord se découpe : il en reste une part, et une part seulement.
///
/// **Le tampon décide**, et l'épreuve existe parce que rien dans la signature ne
/// borne les cotes : un libellé plus large que l'écran est le cas normal à basse
/// résolution.
///
/// **Les deux bornes sont comptées, et non vérifiées sur place** : affirmer que les
/// pixels peints sont dans le tampon ne se falsifierait pas, un relevé ne pouvant lire
/// que ce qui y est. Ce qui se mesure est donc la comparaison avec le même texte au
/// large — strictement moins, et pas rien.
#[test]
fn un_texte_au_bord_se_decoupe() {
    let glyphs = Glyphs::new();

    let mut large = Canvas::new(128, 64);
    glyphs.draw(&mut large.output(), (8, 8), "SCORE", 3, INK);

    let mut serre = Canvas::new(32, 24);
    glyphs.draw(&mut serre.output(), (8, 8), "SCORE", 3, INK);

    let (au_large, au_bord) = (painted(&large).len(), painted(&serre).len());
    assert!(au_bord > 0, "le texte au bord a disparu entièrement");
    assert!(
        au_bord < au_large,
        "le texte au bord peint {au_bord} pixels comme au large : rien n'a été découpé"
    );
}

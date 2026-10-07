// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du disque modulant.
//!
//! **Elles portent sur les texels et non sur l'image rendue** : ce qui décide d'une
//! surface modulée est la valeur qu'elle porte, `255` ne modulant rien et un texel
//! sombre éteignant le tampon. Figer une empreinte d'image mesurerait le moteur une
//! seconde fois, ce que ce dépôt n'a pas à faire.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;

/// Le côté des disques d'épreuve, en texels.
const SIDE: u32 = 32;

/// Ce que le centre des disques d'épreuve laisse passer.
const CORE: f32 = 0x20 as f32;

/// Le texel d'un disque, en niveau de gris.
///
/// **La composante basse suffit, et le disque est gris** : les trois canaux y portent
/// la même valeur, qui est aussi celle que la modulation applique à chacun. Le niveau
/// zéro est celui qu'on engendre, les suivants étant réduits par le moteur.
/// Les coordonnées sont prises en `u32` parce que c'est ainsi qu'on parcourt une
/// texture ; le moteur les veut signées, son repli par masque ramenant aussi les
/// négatives du bon côté.
fn level(texture: &Texture, u: u32, v: u32) -> u8 {
    (texture.texel(0, u as i32, v as i32) & 0xFF) as u8
}

/// Le bord du disque est blanc, donc il ne module rien.
///
/// **C'est la propriété qui l'autorise à être posé sans transparence** : un bord à
/// `255` laisse le tampon intact, là où un bord sombre dessinerait un carré. Les
/// quatre coins sont les points les plus éloignés du centre, donc les premiers à
/// trahir une courbe qui n'atteint pas le blanc.
#[test]
fn le_bord_du_disque_ne_module_rien() {
    let disc = blot(SIDE, CORE);
    let last = SIDE - 1;

    for (u, v) in [(0, 0), (last, 0), (0, last), (last, last)] {
        assert_eq!(
            level(&disc, u, v),
            0xFF,
            "le coin ({u}, {v}) vaut {} et assombrit donc le tampon",
            level(&disc, u, v)
        );
    }
}

/// Le centre porte la densité demandée, et le disque s'éclaircit vers le bord.
///
/// **La monotonie est ce qui fait un dégradé**, et une courbe inversée la perdrait
/// sans que la valeur du centre ni celle du bord ne bougent — les deux seules que
/// l'épreuve précédente regarde. Elle se vérifie le long d'une diagonale, qui
/// traverse toute l'étendue des rayons.
#[test]
fn le_disque_s_eclaircit_du_centre_au_bord() {
    let disc = blot(SIDE, CORE);
    let middle = SIDE / 2;

    // Au texel du centre, le rayon n'est pas nul — il vaut un demi-texel —, donc la
    // densité relevée est très légèrement au-dessus de celle demandée.
    let core = level(&disc, middle, middle);
    assert!(
        f32::from(core) >= CORE && f32::from(core) <= CORE + 2.0,
        "le centre vaut {core} là où {CORE} était demandé"
    );

    let mut previous = core;
    for step in 1..middle {
        let here = level(&disc, middle + step, middle + step);
        assert!(
            here >= previous,
            "le disque s'assombrit de {previous} à {here} en s'éloignant du centre, \
             au pas {step}"
        );
        previous = here;
    }
    assert_eq!(previous, 0xFF, "la diagonale n'atteint pas le blanc");
}

/// Les composantes d'un texel, dans l'ordre que le moteur assemble.
///
/// Il lit les octets d'entrée en petit-boutien, donc le rouge est de poids faible et
/// l'alpha de poids fort — ce dernier étant le seul qui décide, pour une texture
/// masquée, si le texel est peint.
fn rgba(texture: &Texture, u: u32, v: u32) -> [u8; 4] {
    let texel = texture.texel(0, u as i32, v as i32);
    [
        texel as u8,
        (texel >> 8) as u8,
        (texel >> 16) as u8,
        (texel >> 24) as u8,
    ]
}

/// L'éclat porte sa couleur de cœur au centre et celle de bord au rayon.
///
/// **Les deux ensemble, parce qu'une seule ne dit rien** : un éclat uniformément clair
/// passerait le premier point, un éclat uniformément orange le second. C'est le dégradé
/// qui fait lire un impact plutôt qu'une pastille.
#[test]
fn l_eclat_va_du_coeur_au_bord() {
    const CORE: [u8; 3] = [0xFF, 0xF0, 0xC0];
    const EDGE: [u8; 3] = [0xE0, 0x60, 0x10];

    let disc = spark(SIDE, CORE, EDGE);
    let middle = SIDE / 2;

    // Au centre, la couleur du cœur — à un texel près, le centre exact tombant entre
    // quatre texels.
    let core = rgba(&disc, middle, middle);
    for axis in 0..3 {
        assert!(
            core[axis].abs_diff(CORE[axis]) <= 8,
            "au centre, la composante {axis} vaut {} et non {}",
            core[axis],
            CORE[axis]
        );
    }

    // Au bord du disque, sur l'axe horizontal, la couleur de bord : le dernier texel
    // encore peint avant la découpe.
    let rim = rgba(&disc, SIDE - 1, middle);
    assert_eq!(rim[3], 0xFF, "le bord du disque n'est pas peint");
    assert!(
        rim[1] < core[1],
        "le bord est aussi clair que le cœur : {} contre {}",
        rim[1],
        core[1]
    );
}

/// L'éclat est découpé en disque, donc ses coins ne sont pas peints.
///
/// **Sans la découpe, l'impact serait un carré** — et un carré clair sur une silhouette
/// sombre se lit comme un défaut, pas comme un coup. La transparence du moteur étant
/// binaire, c'est l'alpha nul qui découpe, et un texel transparent n'est ni peint ni
/// inscrit dans la profondeur.
#[test]
fn l_eclat_est_decoupe_en_disque() {
    let disc = spark(SIDE, [0xFF, 0xF0, 0xC0], [0xE0, 0x60, 0x10]);
    let last = SIDE - 1;

    for (u, v) in [(0, 0), (last, 0), (0, last), (last, last)] {
        assert_eq!(
            rgba(&disc, u, v)[3],
            0x00,
            "le coin ({u}, {v}) est peint, donc l'éclat est un carré"
        );
    }

    // Et le centre l'est : sans ce second cas, une texture entièrement transparente
    // passerait.
    assert_eq!(rgba(&disc, SIDE / 2, SIDE / 2)[3], 0xFF);
}

/// La densité du centre est ce qui sépare deux disques, à côté égal.
///
/// **Sans elle, le paramètre serait ignoré en silence** : les deux appelants du module
/// ne diffèrent que par le côté et la densité, et un `core` perdu en route donnerait
/// deux images identiques dont l'une est trop claire — ce qui se verrait à l'écran
/// sans qu'on sache où chercher.
#[test]
fn la_densite_demandee_separe_deux_disques() {
    let middle = SIDE / 2;
    let dark = blot(SIDE, 0x10 as f32);
    let pale = blot(SIDE, 0x80 as f32);

    assert!(
        level(&dark, middle, middle) < level(&pale, middle, middle),
        "un centre demandé à 0x10 rend {} et un centre à 0x80 rend {}",
        level(&dark, middle, middle),
        level(&pale, middle, middle)
    );
    // Et les deux gardent leur bord blanc : la densité ne déplace que le centre.
    assert_eq!(level(&dark, 0, 0), 0xFF);
    assert_eq!(level(&pale, 0, 0), 0xFF);
}

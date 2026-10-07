// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du gabarit du joueur et de sa hauteur d'œil.
//!
//! Elles passent par le **chargement** et par le balayage, comme celles de
//! l'export : une pose ne se juge pas sur ses coordonnées mais sur ce que le
//! moteur en dit — dans le solide, ou posée sur quelque chose.
//!
//! **Ce qui relève de la politique de collision est éprouvé avec elle**, dans les
//! épreuves du corps : glissade, chute, collage et franchissement ne parlent pas du
//! joueur, et les laisser ici aurait fait croire qu'ils lui appartiennent.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use crate::test_support::{SEEDS, maze};
use screengine_play::sweep_reach;

/// Le joueur naît debout, jamais dans le solide.
///
/// **Le départ est le centre de la grille et rien n'interdit qu'il tombe sur une
/// cage** : l'empreinte du corps y dépasse le palier et surplombe des marches, ou
/// repose sur une rampe à quarante-cinq degrés. Posé à la seule cote du sol, le
/// corps les pénètre.
///
/// **Elle a attendu un correctif du moteur, et c'est elle qui l'a rendu visible
/// chez nous** : une cellule de case sur trois n'arrêtait rien, donc le balayage
/// qui pose le corps n'y rencontrait pas son sol et le laissait en dessous.
/// `un_mur_plein_arrete_un_pas` porte la mesure qui a désigné la cause.
#[test]
fn le_joueur_nait_hors_du_solide() {
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let player = Player::spawn(&grid, &map);
        let centre = player.body.centre();
        let cell = player.cell();
        assert_ne!(cell, 0, "graine {seed:#x} : le joueur naît hors du décor");

        // Un pas nul ne rencontre rien, par contrat : il faut un mouvement pour
        // que le moteur se prononce, et le plus court qui dise quelque chose est
        // une descente — c'est aussi celle qui porte le poids du corps.
        let below = Vec3::new(centre.x, centre.y, centre.z - 1.0);
        let hit = map
            .sweep(cell, HALF, centre, below)
            .expect("la cellule du joueur existe");
        assert!(
            !hit.start_solid,
            "graine {seed:#x} : le joueur naît dans le solide en {centre:?}, \
             contre la surface {} de normale {:?}",
            hit.surface, hit.normal
        );
    }
}

/// Et il naît **posé** : ce qu'il a sous les pieds l'arrête aussitôt.
///
/// La distinction avec l'épreuve précédente est celle qui sépare un personnage
/// debout d'un personnage qui flotte : la première dit qu'il ne pénètre rien, et
/// seule celle-ci dit qu'il touche. Une pose relevée d'un centimètre de trop les
/// passerait toutes les deux et ferait tomber le joueur à la première image de
/// gravité.
///
/// **Arrêté dans la bande de contact, et non à la fraction zéro exacte.** Le
/// moteur rend une position *juste avant* ce qu'elle touche, donc une sonde reprise
/// de là descend d'un résidu avant de se prononcer — mesuré à deux
/// cent-millièmièmes de millimètre. Ce qui se vérifie est donc la course, qui doit
/// rester sous la dilatation de la boîte : un millième de sa plus grande
/// demi-étendue.
///
/// Elle a attendu le même correctif que l'épreuve précédente, pour la même raison.
#[test]
fn le_joueur_nait_pose_sur_son_sol() {
    /// La course de la sonde, en unités de monde.
    const PROBE: f32 = 1.0;

    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let player = Player::spawn(&grid, &map);
        let centre = player.body.centre();

        let below = Vec3::new(centre.x, centre.y, centre.z - PROBE);
        let hit = map
            .sweep(player.cell(), HALF, centre, below)
            .expect("la cellule du joueur existe");
        assert!(
            hit.fraction * PROBE <= HALF.z / 1024.0,
            "graine {seed:#x} : le joueur descend de {} avant de toucher, \
             depuis {centre:?}",
            hit.fraction * PROBE
        );
        // Le sol, et non un flanc : une normale qui regarde le haut. Plat, en
        // marches ou en rampe, sa composante verticale reste la plus grande.
        assert!(
            hit.normal.z > 0.5,
            "graine {seed:#x} : ce qui le porte a pour normale {:?}",
            hit.normal
        );
    }
}

/// L'œil est au-dessus du centre du corps, et d'une hauteur plausible.
///
/// **Ce n'est pas une paraphrase du calcul** : elle fige le sens de la
/// dérivation, qui est tout ce que l'étape impose — un volume centré sur l'œil
/// flotterait au-dessus de ce qui est bas et le traverserait. Elle attraperait un
/// signe inversé, que rien d'autre ne dirait avant l'écran.
#[test]
fn l_oeil_est_au_dessus_du_corps() {
    let (grid, map) = maze(SEEDS[0]);
    let player = Player::spawn(&grid, &map);
    let centre = player.body.centre();
    let eye = player.eye();

    assert_eq!(eye.x, centre.x);
    assert_eq!(eye.y, centre.y);
    assert_eq!(eye.z, centre.z + EYE_ABOVE);
    assert!(
        eye.z - (centre.z - HALF.z) > 1.0,
        "l'œil est à {} du sol, ce qui n'est pas une taille d'homme",
        eye.z - (centre.z - HALF.z)
    );
}

/// L'œil et le centre du corps tombent dans la même cellule, partout.
///
/// **C'est la précondition du tir, et elle n'est garantie par rien** : le rayon part
/// de l'œil, et le balayage prend une cellule de départ dont il ne vérifie pas
/// qu'elle contienne le point — ce serait un test d'appartenance par requête pour un
/// appelant qui le sait déjà. Or la cellule suivie est celle du **centre**, soixante
/// centimètres plus bas.
///
/// Mesurée sur toutes les cases de toutes les graines, cages comprises : ce sont
/// elles qui empileraient deux cellules dans la hauteur d'un corps si quelque chose
/// le faisait.
#[test]
fn l_oeil_et_le_corps_sont_dans_la_meme_cellule() {
    let mut posed = 0;
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        for at in crate::test_support::cases(&grid) {
            let player = Player::stand(&grid, &map, at);
            let cell = player.cell();
            if cell == 0 {
                continue;
            }
            posed += 1;
            assert_eq!(
                map.locate(player.eye()),
                cell,
                "graine {seed:#x}, case {at:?} : l'œil en {:?} sort de la cellule \
                 du corps, posé en {:?}",
                player.eye(),
                player.body.centre()
            );
        }
    }

    // **Sans ce compte, l'épreuve est verte sur un corpus vide** : une pose hors du
    // décor se saute, et rien ne dirait que toutes se sautent. Le seuil est celui
    // d'une grille de seize sur seize sur deux étages, dont les cases pleines sont
    // la moitié environ, fois six graines.
    assert!(
        posed > 2000,
        "seules {posed} poses ont été éprouvées, le corpus n'en est pas un"
    );
}

/// Le décor joué tient là où la boîte du joueur garde son jeu de collision.
///
/// **Une épreuve de dimensionnement, et elle est verte d'avance** : le balayage
/// laisse à la boîte un jeu qui se perd avec l'éloignement de l'origine, et une
/// grille de seize cases est à deux ordres de grandeur en dessous. Ce qu'elle garde
/// est donc l'avenir — une grille plus large, ou une boîte plus petite qu'on
/// balaierait —, et c'est pourquoi elle part du réglage **joué** et non de celui des
/// épreuves : c'est lui qu'on agrandit.
///
/// **La distance s'appelle et ne se recopie pas**, comme la marge : elle dérive
/// d'une constante qui ne fait pas partie du contrat, et un seuil recopié se
/// tromperait sur exactement les boîtes où il décide.
#[test]
fn le_decor_joue_garde_le_jeu_de_la_boite() {
    let (width, height, levels) = crate::MAZE.extent;

    // L'export pose la grille depuis l'origine, donc la plus grande coordonnée du
    // décor est celle du coin opposé — en hauteur, le plafond du dernier étage.
    let far = (width as f32 * export::CELL)
        .max(height as f32 * export::CELL)
        .max((levels - 1) as f32 * export::LEVEL + export::CEILING);

    // Strictement : à cette distance le jeu est déjà perdu, c'est le seuil et non
    // la dernière cote sûre.
    assert!(
        far < sweep_reach(HALF),
        "le décor va jusqu'à {far}, et la boîte du joueur perd son jeu à {}",
        sweep_reach(HALF)
    );
}

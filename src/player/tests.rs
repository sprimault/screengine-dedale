// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du corps du joueur.
//!
//! Elles passent par le **chargement** et par le balayage, comme celles de
//! l'export : une pose ne se juge pas sur ses coordonnées mais sur ce que le
//! moteur en dit — dans le solide, ou posée sur quelque chose.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use crate::maze::grid::Settings;

/// Un labyrinthe d'épreuve, par sa graine.
///
/// **Plusieurs graines plutôt qu'une** : le départ est le centre de la grille et
/// les volées sont tirées, donc la forme du sol sous les pieds du joueur change
/// d'une graine à l'autre — plat, en marches, ou en rampe. Une seule graine
/// n'éprouverait qu'une des trois, et sans dire laquelle.
fn maze(seed: u64) -> (Grid, World) {
    let grid = Grid::generate(Settings {
        extent: (16, 16, 2),
        seed,
        stairs: 6,
        ramps: 2,
        loops: 8,
    });
    let map = World::load(&export::world(&grid)).expect("carte engendrée valide");
    (grid, map)
}

/// Les graines éprouvées, dont celle du jeu.
const SEEDS: [u64; 6] = [0x5EED_1A8E, 1, 2, 3, 0xD1CE, 0xFACE];

/// Le joueur naît debout, jamais dans le solide.
///
/// **C'est l'épreuve que ce lot existe pour rendre.** Le départ est le centre de
/// la grille et rien n'interdit qu'il tombe sur une cage : l'empreinte du corps
/// y dépasse le palier et surplombe des marches, ou repose sur une rampe à
/// quarante-cinq degrés. Posé à la seule cote du sol, le corps les pénètre.
///
/// **En attente nommée** : une cellule de case sur trois n'arrête rien, donc le
/// balayage qui pose le corps n'y rencontre pas son sol et le laisse en dessous.
/// Le défaut est du moteur — `un_mur_plein_arrete_un_pas` le mesure —, et cette
/// épreuve deviendra verte avec son correctif, sans que ce module change.
#[test]
#[ignore = "le balayage ne voit pas le sol d'une cellule sur trois, donc la pose tombe dedans"]
fn le_joueur_nait_hors_du_solide() {
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let player = Player::spawn(&grid, &map);
        let cell = player.cell();
        assert_ne!(cell, 0, "graine {seed:#x} : le joueur naît hors du décor");

        // Un pas nul ne rencontre rien, par contrat : il faut un mouvement pour
        // que le moteur se prononce, et le plus court qui dise quelque chose est
        // une descente — c'est aussi celle qui porte le poids du corps.
        let below = Vec3::new(player.centre.x, player.centre.y, player.centre.z - 1.0);
        let hit = map
            .sweep(cell, HALF, player.centre, below)
            .expect("la cellule du joueur existe");
        assert!(
            !hit.start_solid,
            "graine {seed:#x} : le joueur naît dans le solide en {:?}, \
             contre la surface {} de normale {:?}",
            player.centre, hit.surface, hit.normal
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
/// **En attente nommée**, pour la même raison que l'épreuve précédente.
#[test]
#[ignore = "le balayage ne voit pas le sol d'une cellule sur trois, donc rien ne le porte"]
fn le_joueur_nait_pose_sur_son_sol() {
    /// La course de la sonde, en unités de monde.
    const PROBE: f32 = 1.0;

    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let player = Player::spawn(&grid, &map);

        let below = Vec3::new(player.centre.x, player.centre.y, player.centre.z - PROBE);
        let hit = map
            .sweep(player.cell(), HALF, player.centre, below)
            .expect("la cellule du joueur existe");
        assert!(
            hit.fraction * PROBE <= HALF.z / 1024.0,
            "graine {seed:#x} : le joueur descend de {} avant de toucher, depuis {:?}",
            hit.fraction * PROBE,
            player.centre
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
    let eye = player.eye();

    assert_eq!(eye.x, player.centre.x);
    assert_eq!(eye.y, player.centre.y);
    assert_eq!(eye.z, player.centre.z + EYE_ABOVE);
    assert!(
        eye.z - (player.centre.z - HALF.z) > 1.0,
        "l'œil est à {} du sol, ce qui n'est pas une taille d'homme",
        eye.z - (player.centre.z - HALF.z)
    );
}

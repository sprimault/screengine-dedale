// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du relevé.

use super::*;
use crate::test_support::{Canvas, HEIGHT, WIDTH};

/// Une caméra libre posée et orientée de travers, comme une marche la laisse.
///
/// Des valeurs qui ne sont ni nulles ni remarquables : un lacet de quart de tour
/// ou un tangage nul passeraient une composition fausse sans rien dire.
fn camera() -> FreeCamera {
    FreeCamera {
        position: Vec3::new(55.56, 13.83, 5.00),
        yaw: 2.317,
        pitch: -0.184,
        ..FreeCamera::new(Vec3::new(55.56, 13.83, 5.00))
    }
}

/// Une pose relevée se rejoue à l'identique.
///
/// **C'est la propriété pour laquelle le relevé note l'orientation**, et elle ne
/// vaut que si rien ne se perd en route : une épreuve qui repose le lacet et le
/// tangage dans une `FreeCamera` doit obtenir l'orientation que le jeu avait, au
/// bit près. Approchée, elle ne reproduirait pas un défaut qui tient à des
/// centièmes d'unité.
#[test]
fn une_pose_relevee_se_rejoue_a_l_identique() {
    let marche = camera();
    let aim = Aim::new(&marche, 1147);

    let rejouee = FreeCamera {
        position: aim.eye,
        yaw: aim.yaw,
        pitch: aim.pitch,
        ..FreeCamera::new(aim.eye)
    };

    assert_eq!(rejouee.camera().position, marche.camera().position);
    assert_eq!(rejouee.camera().orientation, marche.camera().orientation);
}

/// La ligne du relevé donne les angles en degrés.
///
/// **Ils se relisent à l'œil pour se situer dans un couloir**, et c'est la seule
/// raison de les convertir : un lacet de 2,317 radians ne se reconnaît pas, 132,75°
/// se place. L'épreuve tient la conversion autant que le gabarit, parce qu'une
/// ligne illisible ne se remarque pas quand on en relit quatre-vingts.
#[test]
fn la_ligne_donne_les_angles_en_degres() {
    let ligne = Aim::new(&camera(), 1147).to_string();
    assert_eq!(
        ligne,
        "œil (55.56 13.83 5.00), lacet 132.75°, tangage -10.54°, cellule 1147"
    );
}

/// Un cap accumulé hors du tour s'imprime dedans, et la pose rangée n'y revient
/// pas.
///
/// **Le cas est relevé, non construit** : la marche du 2026-10-08 a fini sur
/// `lacet -318.14°`, qui est le cap de 41,86° et se relit par une soustraction
/// mentale de 360. Le lacet d'une caméra libre s'accumule sans borne, donc un
/// tour complet dans un sens suffit à l'en sortir.
///
/// **Les deux assertions sont les deux moitiés de l'arbitrage** : ce qui se ramène
/// est l'impression, jamais la pose.
///
/// **Et la seconde ne fait pas double emploi avec
/// [`une_pose_relevee_se_rejoue_a_l_identique`]** : celle-ci reste verte quand on
/// normalise le lacet rangé, mesuré en la falsifiant. Un angle décalé d'un tour
/// entier recompose la même orientation, donc une comparaison d'orientations ne
/// peut pas voir la normalisation — seule l'égalité du champ la voit.
#[test]
fn un_cap_hors_du_tour_s_imprime_dedans() {
    let turned = (-318.14_f32).to_radians();
    let aim = Aim::new(
        &FreeCamera {
            yaw: turned,
            ..camera()
        },
        1147,
    );

    let ligne = aim.to_string();
    assert!(
        ligne.contains("lacet 41.86°"),
        "le cap ne revient pas dans le tour : {ligne}"
    );
    assert_eq!(
        aim.yaw, turned,
        "la pose rangée a été normalisée, donc elle ne se rejoue plus au bit"
    );
}

/// Le nombre de tours accumulés ne change pas la ligne.
///
/// **C'est la propriété, et non le seul cas relevé** : un relevé qui dépendrait du
/// nombre de tours donnerait deux lignes différentes pour le même cap, ce qui est
/// précisément ce qu'on ne peut pas voir en relisant quatre-vingts lignes. Trois
/// tours de part et d'autre suffisent — au-delà, c'est la précision du `f32` qui
/// se mesurerait, pas la conversion.
#[test]
fn le_cap_ne_depend_pas_du_nombre_de_tours() {
    let attendue = Aim::new(&camera(), 1147).to_string();

    for turns in -3..=3 {
        let yaw = camera().yaw + turns as f32 * core::f32::consts::TAU;
        let ligne = Aim::new(&FreeCamera { yaw, ..camera() }, 1147).to_string();
        assert_eq!(ligne, attendue, "{turns} tours changent la ligne");
    }
}

/// Une teinte, complétée de son octet d'alpha.
///
/// Le relevé ne compare que les trois premiers octets ; celui-ci est le bourrage que la
/// recopie vers la fenêtre jette, et il est posé pour que le tampon d'épreuve ressemble
/// à ce que le moteur laisse derrière lui.
fn opaque(colour: [u8; 3]) -> [u8; 4] {
    [colour[0], colour[1], colour[2], 0xFF]
}

/// Un tampon peint d'une seule teinte, et ce que le relevé y compte.
fn tally(fill: [u8; 3], background: [u8; 3]) -> u32 {
    let mut canvas = Canvas::new(WIDTH, HEIGHT);
    canvas.fill(opaque(fill));
    painted(&mut canvas.output(), background)
}

/// Une image entièrement au fond ne compte aucun point peint, quelle que soit la
/// couleur du fond.
///
/// **C'est le défaut que le relevé portait** : il comparait au noir, si bien qu'un
/// fond de brouillard — la couleur qu'un pixel non peint prend de lui-même, dit le
/// contrat du moteur — aurait compté pour une image pleine. L'instrument serait
/// devenu muet le jour où l'ambiance arrive, sans rien dire.
#[test]
fn une_image_au_fond_ne_compte_rien() {
    for background in [[0x00, 0x00, 0x00], [0x30, 0x34, 0x3C], [0xFF, 0xFF, 0xFF]] {
        assert_eq!(
            tally(background, background),
            0,
            "un fond {background:?} compte des points peints"
        );
    }
}

/// Une image entièrement peinte les compte tous.
#[test]
fn une_image_pleine_compte_tout() {
    assert_eq!(tally([0x80, 0x40, 0x20], [0x00, 0x00, 0x00]), 100);
    assert_eq!(tally([0x00, 0x00, 0x00], [0x30, 0x34, 0x3C]), 100);
}

/// Une image coupée en deux en compte la moitié.
///
/// **C'est la proportion qui est en jeu**, et non le tout ou rien : la coupure que
/// le relevé existe pour voir laisse l'autre moitié intacte, qu'un seuil « image
/// entièrement vide » ne verrait jamais.
#[test]
fn une_image_coupee_en_compte_la_moitie() {
    let background = [0x30, 0x34, 0x3C];

    let mut canvas = Canvas::new(WIDTH, HEIGHT);
    canvas.fill(opaque(background));
    for y in 0..HEIGHT {
        for x in 0..WIDTH / 2 {
            canvas.set(x, y, opaque([0x80, 0x40, 0x20]));
        }
    }

    assert_eq!(painted(&mut canvas.output(), background), 50);
}

/// Le relevé voit l'interface, donc l'ordre du rappel de sortie n'est pas indifférent.
///
/// **Ce qui est éprouvé est la conséquence, parce que l'ordre lui-même ne s'éprouve
/// pas d'ici** : il vit dans le rappel de sortie, dont l'argument est une session que
/// rien ne fabrique sans ouvrir le fichier de relevé. Ce qui se mesure est que
/// l'interface compte comme un pixel peint — donc que la dessiner avant le relevé le
/// rendrait aveugle à une image amputée d'autant.
///
/// **Et le coût est figé plutôt que borné** : un plafond au seuil du relevé ne se
/// falsifierait pas — l'interface ne peut pas l'atteindre, ses marges l'empêchant de
/// couvrir toute la largeur quelles que soient ses cotes. Ce qui est figé est donc ce
/// qu'elle coûte vraiment, et l'épreuve rougit le jour où l'interface grandit. C'est
/// le moment où il faut relire l'ordre du rappel, pas avant.
///
/// **Les deux éléments permanents du bas de l'écran y sont**, et non la jauge seule :
/// ce qui intéresse l'ordre du rappel est le total, et un élément ajouté sans entrer
/// ici ferait croire que le coût n'a pas bougé. L'épitaphe, elle, n'en fait pas partie
/// — elle ne s'écrit que la course finie, quand il n'y a plus d'épisode à relever.
///
/// **Quatre points, dont trois pour le compteur**, mesuré en lui donnant sa plaque : il
/// est trois fois plus coûteux que la jauge alors qu'il couvre moins de pixels, parce
/// que les deux cent cinquante-six points du relevé sont répartis en grille et qu'un
/// bandeau de sept pixels de haut en traverse moins de rangées qu'une plaque de vingt.
/// Reste très loin du seuil, et c'est ce que l'écart dit.
#[test]
fn le_releve_voit_l_interface() {
    let background = [0x30, 0x34, 0x3C];
    let mut canvas = Canvas::new(WIDTH, HEIGHT);
    canvas.fill(opaque(background));

    assert_eq!(
        painted(&mut canvas.output(), background),
        0,
        "le fond seul compte des points peints"
    );

    crate::hud::gauge::draw(&mut canvas.output(), 1.0);
    let gauge = painted(&mut canvas.output(), background);
    assert!(
        gauge > 0,
        "le relevé ne voit pas la jauge, donc l'ordre du rappel serait indifférent"
    );

    let glyphs = crate::hud::glyph::Glyphs::new();
    crate::hud::score::draw(&mut canvas.output(), &glyphs, 0);
    let after = painted(&mut canvas.output(), background);

    assert!(
        after > gauge,
        "le relevé ne voit pas le compteur, qu'il compte pourtant comme peint"
    );
    assert_eq!(
        after, 4,
        "l'interface coûte {after} points au relevé et non quatre : elle a grandi, \
         donc l'ordre du rappel de sortie est à relire contre le seuil de {THRESHOLD}"
    );
}

// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du découpage d'une planche de vues.
//!
//! **Tout y est une fonction d'entrées vers des sorties**, donc tout s'éprouve
//! sans ouvrir de fenêtre — ce qui est rare dans ce dépôt et vaut d'être employé.
//!
//! **Aucune comparaison de deux implémentations du même calcul d'angle.** Un
//! oracle ne voit pas ce que les deux chemins partagent, et ce dépôt l'a payé
//! trois fois. Ce qui attrape un découpage faux est un **prédicat** : le secteur
//! retenu contient son angle, et deux angles écartés de plus d'un secteur ne
//! rendent jamais la même vue.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use screengine_play::{Texture, load_png_masked};

/// L'angle que couvre une vue, en radians.
const SECTOR: f32 = core::f32::consts::TAU / VIEWS as f32;

/// Des caps d'épreuve, en radians.
///
/// **Plusieurs plutôt qu'un seul, et aucun aligné sur un axe** : le choix de la
/// vue est un écart, donc un cap nul laisserait passer un code qui lirait la
/// direction de l'œil au lieu de la différence. Les deux derniers sortent du tour
/// pour exercer le repli.
const FACINGS: [f32; 5] = [0.0, 0.7, 2.9, -1.3, 8.1];

/// Des poses d'épreuve, pour que rien ne dépende de l'origine du monde.
const PLACES: [Vec3; 3] = [
    Vec3::new(0.0, 0.0, 0.0),
    Vec3::new(37.5, -12.25, 3.5),
    Vec3::new(-4.0, 60.0, -7.0),
];

/// Les neuf planches du dépôt, avec le cycle que chacune porte.
///
/// **Intégrées ici et non dans le module**, qui ne connaît aucune créature : ce
/// que l'épreuve confronte est la grille annoncée aux **fichiers livrés**, pas ce
/// qu'un autre module en fera.
const SHEETS: [(&str, &[u8], Motion); 9] = [
    (
        "d1-idle",
        include_bytes!("../../assets/sprites/demon-d1-idle-8x8-64.png"),
        Motion::Idle,
    ),
    (
        "d1-walk",
        include_bytes!("../../assets/sprites/demon-d1-walk-8x8-64.png"),
        Motion::Walk,
    ),
    (
        "d1-mort",
        include_bytes!("../../assets/sprites/demon-d1-mort-8x16-64.png"),
        Motion::Dead,
    ),
    (
        "d2-idle",
        include_bytes!("../../assets/sprites/demon-d2-idle-8x8-64.png"),
        Motion::Idle,
    ),
    (
        "d2-walk",
        include_bytes!("../../assets/sprites/demon-d2-walk-8x8-64.png"),
        Motion::Walk,
    ),
    (
        "d2-mort",
        include_bytes!("../../assets/sprites/demon-d2-mort-8x16-64.png"),
        Motion::Dead,
    ),
    (
        "d3-idle",
        include_bytes!("../../assets/sprites/demon-d3-idle-8x8-64.png"),
        Motion::Idle,
    ),
    (
        "d3-walk",
        include_bytes!("../../assets/sprites/demon-d3-walk-8x8-64.png"),
        Motion::Walk,
    ),
    (
        "d3-mort",
        include_bytes!("../../assets/sprites/demon-d3-mort-8x16-64.png"),
        Motion::Dead,
    ),
];

/// Où poser l'œil pour qu'il voie la créature sous cet écart.
///
/// La distance ne décide de rien — seule la direction compte —, mais elle n'est
/// pas unitaire pour autant : une longueur de un laisserait passer un code qui
/// aurait oublié de retrancher la pose de la créature.
fn eye(at: Vec3, facing: f32, relative: f32) -> Vec3 {
    // **L'écart se retranche du cap**, parce que c'est ainsi que les vues tournent
    // sur nos planches : l'œil qui donne l'écart `relative` se place donc en
    // `facing − relative`, et non à sa somme.
    let bearing = facing - relative;
    Vec3::new(at.x + 6.5 * bearing.cos(), at.y + 6.5 * bearing.sin(), at.z)
}

/// L'écart le plus court entre deux angles, en radians.
fn gap(a: f32, b: f32) -> f32 {
    let turn = core::f32::consts::TAU;
    let d = (a - b).rem_euclid(turn);
    d.min(turn - d)
}

/// L'œil droit devant la créature la montre de face.
///
/// **C'est l'ancre de toute la convention** : si la ligne 0 n'était pas la vue de
/// face, chaque autre épreuve resterait verte en décrivant une planche tournée
/// d'un cran. Elle vaut pour n'importe quel cap, et c'est ce qui dit que la vue
/// est un écart.
#[test]
fn l_oeil_droit_devant_rend_la_vue_de_face() {
    for facing in FACINGS {
        for at in PLACES {
            assert_eq!(
                row(facing, at, eye(at, facing, 0.0)),
                0,
                "cap {facing}, pose {at:?} : l'œil droit devant ne rend pas la vue \
                 de face"
            );
        }
    }
}

/// L'œil derrière elle la montre de dos.
///
/// **La seconde ancre, et elle n'est pas redondante** : la première tient encore
/// si les lignes tournent à l'envers, puisque l'aller et le retour passent tous
/// deux par zéro. Celle-ci fixe le demi-tour au milieu de la planche, donc
/// l'espacement régulier des huit vues.
#[test]
fn l_oeil_derriere_rend_la_vue_de_dos() {
    for facing in FACINGS {
        for at in PLACES {
            assert_eq!(
                row(facing, at, eye(at, facing, core::f32::consts::PI)),
                VIEWS / 2,
                "cap {facing}, pose {at:?} : l'œil derrière ne rend pas la vue de dos"
            );
        }
    }
}

/// Le secteur retenu contient son angle, partout dans sa largeur.
///
/// **C'est le prédicat du lot**, et il attrape ce qu'aucune comparaison de deux
/// implémentations ne verrait : une troncature au lieu d'un arrondi décale chaque
/// vue d'un demi-secteur, et les deux chemins comparés se décaleraient ensemble.
///
/// **Les bords s'approchent sans se toucher** : à la frontière exacte, le secteur
/// retenu est celui que l'arrondi du `f32` décide, et une épreuve posée là
/// mesurerait cet arrondi plutôt que le découpage.
#[test]
fn le_secteur_retenu_contient_son_angle() {
    /// Où l'on se place dans le secteur, en fraction de sa largeur depuis son
    /// centre.
    const WITHIN: [f32; 5] = [-0.49, -0.25, 0.0, 0.25, 0.49];

    for facing in FACINGS {
        for at in PLACES {
            for view in 0..VIEWS {
                let centre = view as f32 * SECTOR;
                for part in WITHIN {
                    let relative = centre + part * SECTOR;
                    assert_eq!(
                        row(facing, at, eye(at, facing, relative)),
                        view,
                        "cap {facing}, pose {at:?} : à {part} de secteur du centre \
                         de la vue {view}, c'est une autre vue qui sort"
                    );
                }
            }
        }
    }
}

/// Deux angles écartés de plus d'un secteur ne rendent jamais la même vue.
///
/// **L'autre moitié du prédicat** : la première dit qu'un secteur est assez large,
/// celle-ci qu'il n'est pas trop large. Un découpage qui rendrait la même vue sur
/// un tour entier passerait la première sans broncher.
///
/// Le balayage est au degré, soit un huitième de secteur : assez fin pour qu'aucun
/// secteur ne soit sauté, et assez loin des frontières pour que l'arrondi du `f32`
/// n'y décide de rien.
#[test]
fn deux_angles_ecartes_d_un_secteur_changent_de_vue() {
    /// De combien on avance entre deux angles éprouvés, en radians.
    const STEP: f32 = core::f32::consts::TAU / 360.0;
    /// Ce qu'on laisse à la frontière pour n'y rien mesurer, en radians.
    const MARGIN: f32 = 1.0e-3;

    let facing = FACINGS[1];
    let at = PLACES[1];
    let mut seen = 0;

    for i in 0..360 {
        for j in 0..360 {
            let (a, b) = (i as f32 * STEP, j as f32 * STEP);
            if gap(a, b) <= SECTOR + MARGIN {
                continue;
            }
            assert_ne!(
                row(facing, at, eye(at, facing, a)),
                row(facing, at, eye(at, facing, b)),
                "les angles {a} et {b}, écartés de {}, rendent la même vue",
                gap(a, b)
            );
            seen += 1;
        }
    }

    assert!(seen > 0, "aucune paire assez écartée n'a été éprouvée");
}

/// La cote ne décide d'aucune vue.
///
/// **C'est la conséquence de l'orientation axiale, et elle s'éprouve** : le moteur
/// ne fait tourner le quadrilatère qu'autour du Z du monde, donc une créature
/// regardée d'en haut montre la même vue qu'à hauteur d'œil. Un code qui prendrait
/// l'écart dans l'espace au lieu de sa projection le ferait changer de ligne quand
/// on monte un escalier.
#[test]
fn la_vue_ne_depend_pas_de_la_cote() {
    for facing in FACINGS {
        for at in PLACES {
            for view in 0..VIEWS {
                let level = eye(at, facing, view as f32 * SECTOR);
                for lift in [-9.0, -0.5, 0.5, 9.0] {
                    let raised = Vec3::new(level.x, level.y, level.z + lift);
                    assert_eq!(
                        row(facing, at, raised),
                        row(facing, at, level),
                        "cap {facing}, pose {at:?} : l'œil relevé de {lift} change \
                         la vue {view}"
                    );
                }
            }
        }
    }
}

/// Un cycle qui boucle parcourt ses trames et reprend à zéro.
///
/// **Deux exigences, et la seconde est celle qui compte** : qu'il passe par toutes
/// ses trames dans l'ordre, et qu'un tour de plus retombe exactement sur la même.
/// Sans la seconde, un cycle pourrait s'arrêter sur sa dernière trame sans que rien
/// ne le dise.
#[test]
fn un_cycle_qui_boucle_reprend_a_zero() {
    for motion in [Motion::Idle, Motion::Walk] {
        let frames = motion.frames();

        for frame in 0..frames {
            // Le milieu de la trame, pour ne pas mesurer l'arrondi de sa frontière.
            let phase = (frame as f32 + 0.5) / frames as f32;
            assert_eq!(
                column(motion, phase),
                frame,
                "{motion:?} : la phase {phase} ne rend pas la trame {frame}"
            );
            for turns in [1.0, 2.0, 17.0] {
                assert_eq!(
                    column(motion, phase + turns),
                    frame,
                    "{motion:?} : {turns} tours plus tard, la trame {frame} a changé"
                );
            }
        }
    }
}

/// La mort ne boucle pas : elle garde sa dernière trame.
///
/// **Rien dans la planche ne le dit**, et c'est tout l'objet de cette épreuve :
/// seize images d'une créature qui tombe se rejoueraient indéfiniment, et un
/// cadavre se relèverait toutes les deux secondes. Ce qui est exigé est que le
/// cycle parcoure ses seize trames, puis n'en bouge plus — un tour plus tard comme
/// mille.
#[test]
fn la_mort_garde_sa_derniere_trame() {
    let frames = Motion::Dead.frames();
    let last = frames - 1;

    for frame in 0..frames {
        let phase = (frame as f32 + 0.5) / frames as f32;
        assert_eq!(
            column(Motion::Dead, phase),
            frame,
            "la phase {phase} ne rend pas la trame {frame} du cycle de mort"
        );
    }

    for phase in [1.0, 1.5, 2.0, 1000.0] {
        assert_eq!(
            column(Motion::Dead, phase),
            last,
            "à la phase {phase}, le cycle de mort a quitté sa dernière trame"
        );
    }
}

/// Le rectangle couvre sa vignette, et aucune autre.
///
/// **Ce qu'elle attrape est un décalage d'une vignette**, qui ne se verrait
/// autrement qu'à l'écran et seulement sur certaines vues : le rectangle part du
/// coin de sa case, fait exactement un côté dans chaque sens, et reste dans la
/// planche jusqu'à sa dernière ligne et sa dernière colonne.
#[test]
fn le_rectangle_couvre_sa_vignette() {
    for motion in [Motion::Idle, Motion::Walk, Motion::Dead] {
        let frames = motion.frames();
        let (wide, high) = (frames as f32 * FRAME, VIEWS as f32 * FRAME);

        for view in 0..VIEWS {
            for frame in 0..frames {
                let (u0, v0, u1, v1) = rect(view, frame);

                assert_eq!(
                    u1 - u0,
                    FRAME,
                    "la vignette {view}/{frame} n'est pas carrée"
                );
                assert_eq!(
                    v1 - v0,
                    FRAME,
                    "la vignette {view}/{frame} n'est pas carrée"
                );
                assert_eq!(u0, frame as f32 * FRAME, "colonne {frame} mal placée");
                assert_eq!(v0, view as f32 * FRAME, "ligne {view} mal placée");
                assert!(
                    u1 <= wide && v1 <= high,
                    "{motion:?} : la vignette {view}/{frame} sort de la planche"
                );
            }
        }
    }
}

/// La gouttière minimale qu'une vignette garde sur ses quatre bords, en texels.
///
/// **Elle se mesure plutôt que de se lire dans le nom d'un fichier**, et le relevé
/// donne le pire cas : deux texels en haut et en bas sur les trois silhouettes,
/// trois à onze sur les côtés. C'est donc deux qui décide, et c'est cette valeur
/// qu'une planche future ne doit pas descendre.
const GUTTER: u32 = 2;

/// Combien de texels une vignette garde de vide entre sa silhouette et un bord.
///
/// Rend `(gauche, droite, haut, bas)`. Une vignette entièrement vide rendrait son
/// côté quatre fois, ce qui ne se produit sur aucune planche du dépôt.
fn gutter(sheet: &Texture, view: u32, frame: u32) -> (u32, u32, u32, u32) {
    let side = FRAME as u32;
    let (bu, bv) = (frame * side, view * side);
    let solid = |du: u32, dv: u32| sheet.texel(0, (bu + du) as i32, (bv + dv) as i32) >> 24 != 0;

    // Le premier rang non vide depuis chaque bord : la distance cherchée est son
    // indice, puisqu'on compte les rangs entièrement transparents qui précèdent.
    let scan = |pick: &dyn Fn(u32, u32) -> bool| {
        (0..side)
            .find(|&d| (0..side).any(|o| pick(d, o)))
            .unwrap_or(side)
    };

    (
        scan(&|d, o| solid(d, o)),
        scan(&|d, o| solid(side - 1 - d, o)),
        scan(&|d, o| solid(o, d)),
        scan(&|d, o| solid(o, side - 1 - d)),
    )
}

/// Chaque vignette garde une gouttière, et c'est ce qui l'empêche d'emprunter à sa
/// voisine.
///
/// **Elle existe parce que le moteur n'en réserve aucune pour l'hôte**, et il n'a
/// pas à le faire : il ne sait pas qu'un atlas porte des vignettes. Ce que le
/// contrat publie suffit à en déduire le besoin — les mipmaps sont engendrés
/// jusqu'à `1×1` par moyenne des texels, la réduction d'un format masqué pondère le
/// RGB par l'alpha, et le RGB est dilaté sous les texels transparents. Une planche
/// collée bord à bord verrait donc ses vues se mêler dès les niveaux grossiers.
///
/// **Le seuil, mesuré** : la gouttière vaut deux texels au plus serré, donc elle
/// tient entière au niveau 0, vaut un texel au niveau 1, et tombe **sous un texel
/// au niveau 2** — où une vignette ne fait plus que seize texels de côté. C'est de
/// là qu'un bord pourrait emprunter, et seulement de là.
///
/// **Et la réduction elle-même ne mêle rien avant le niveau 7** : la vignette fait
/// soixante-quatre texels, une puissance de deux, donc chaque réduction reste dans
/// ses frontières jusqu'à ce qu'elle vaille un seul texel, au niveau 6. Ce qui est
/// en jeu ici est le filtrage, pas la réduction.
#[test]
fn chaque_vignette_garde_sa_gouttiere() {
    for (name, bytes, motion) in SHEETS {
        let sheet = load_png_masked(bytes).expect("planche du dépôt valide");

        for view in 0..VIEWS {
            for frame in 0..motion.frames() {
                let (left, right, top, bottom) = gutter(&sheet, view, frame);
                let worst = left.min(right).min(top).min(bottom);

                assert!(
                    worst >= GUTTER,
                    "{name}, vue {view}, trame {frame} : gouttière de {worst} texel(s) \
                     — gauche {left}, droite {right}, haut {top}, bas {bottom}"
                );
            }
        }
    }
}

/// Les neuf planches du dépôt tiennent la grille que ce module annonce.
///
/// **C'est elle qui empêche la grille de mentir**, et elle éprouve les fichiers
/// plutôt que leur nom : une planche que la chaîne rendrait sur douze trames, ou
/// sur seize vues, passerait toutes les épreuves ci-dessus et ne se verrait qu'à
/// l'écran, décalée d'une vignette.
///
/// **Elle charge par le même chemin que le jeu**, en texture masquée : un fichier
/// que le moteur refuserait — un côté qui ne serait pas une puissance de deux —
/// échoue ici plutôt qu'au premier lancement.
#[test]
fn les_planches_tiennent_la_grille_annoncee() {
    for (name, bytes, motion) in SHEETS {
        let sheet = load_png_masked(bytes).expect("planche du dépôt valide");

        assert_eq!(
            sheet.width() as f32,
            motion.frames() as f32 * FRAME,
            "{name} : {} de large pour {} trames de {FRAME}",
            sheet.width(),
            motion.frames()
        );
        assert_eq!(
            sheet.height() as f32,
            VIEWS as f32 * FRAME,
            "{name} : {} de haut pour {VIEWS} vues de {FRAME}",
            sheet.height()
        );
    }
}
